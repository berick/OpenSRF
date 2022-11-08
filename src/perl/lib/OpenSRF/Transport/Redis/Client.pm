package OpenSRF::Transport::Redis::Client;
use strict;
use warnings;
use Redis;
use Time::HiRes q/time/;
use OpenSRF::Utils::JSON;
use OpenSRF::Utils::Logger qw/$logger/;
use OpenSRF::Transport;
use OpenSRF::Transport::Redis::Message;
use OpenSRF::Transport::Redis::BusConnection;

# Map of bus node names to bus connections.
my %connections;

# There will only be one Client per process, but each client may
# have multiple connections.
my $_singleton;
sub retrieve { return $_singleton; }

sub new {
    my ($class, $connection_type, $service, $no_cache) = @_;

    return $_singleton if $_singleton && !$no_cache;

    $logger->debug("Creating new Redis::Client with type '$connection_type'");

    my $self = {
        service => $service,
        connection_type => $connection_type
    };

    bless($self, $class);

    my $pc = OpenSRF::Utils::Conf->current->primary_connection
        || die "Primary connection required\n";

    my $node_name = $pc->node->name;

    # Create a connection for our primary node.
    $self->add_connection($node_name);
    $self->{primary_node_name} = $node_name;

    if ($service) {
        # If we're a service, this is where we listen for service-level requests.
        $self->{service_address} = "opensrf:service:$service";
        $self->create_service_stream;
    }

    $_singleton = $self unless $no_cache;

    return $self;
}

sub reset {                                                                    
    return unless $_singleton;
    $logger->debug("Redis client disconnecting on reset()");
    $_singleton->disconnect;
    $_singleton = undef;
} 

sub connection_type {
    my $self = shift;
    return $self->{connection_type};
}

sub add_connection {
    my ($self, $node_name) = @_;

    my $conf = OpenSRF::Utils::Conf->current;
    my $pc = $conf->primary_connection;
    my $ctype = $pc->connection_type;
    my $node = $conf->get_node($node_name);

    # Assumes other connection parameters are the same across
    # Redis instances, apart from the hostname.
    my $connection = OpenSRF::Transport::Redis::BusConnection->new(
        $node->name,
        $node->port, 
        $ctype->credentials->username, 
        $ctype->credentials->password,
        $node->max_queue_length
    );

    $connection->set_address($self->service);
    $connections{$node_name} = $connection;
    
    $connection->connect;

    return $connection;
}

sub get_connection {
    my ($self, $node_name) = @_;

    my $con = $connections{$node_name};

    return $con if $con;

    eval { $con = $self->add_connection($node_name) };

    if ($@) {
        $logger->error("Could not connect to bus on node: $node_name : $@");
        return undef;
    }

    return $con;
}

# Contains a value if this is a service client.
# Undef for standalone clients.
sub service {
    my $self = shift;
    return $self->{service};
}

# Contains a value if this is a service client.
# Undef for standalone clients.
sub service_address {
    my $self = shift;
    return $self->{service_address};
}

sub primary_node_name {
    my $self = shift;
    return $self->{primary_node_name};
}

sub primary_connection {
    my $self = shift;
    return $connections{$self->primary_node_name};
}

sub disconnect {
    my ($self, $node_name) = @_;

    for my $node_name (keys %connections) {
        my $con = $connections{$node_name};
        $con->disconnect($self->primary_node_name eq $node_name);
        delete $connections{$node_name};
    }
}

sub connected {
    my $self = shift;
    return $self->primary_connection && $self->primary_connection->connected;
}

sub tcp_connected {
    my $self = shift;
    return $self->connected;
}

sub create_service_stream {
    my $self = shift;

    eval { 
        # This gets mad when a stream / group already exists, 
        # but it's conceivable that it's already been created.

        $self->primary_connection->redis->xgroup(   
            'create',
            $self->service_address, # stream name
            $self->service_address, # group name
            '$',            # only receive new messages
            'mkstream'      # create this stream if it's not there.
        );
    };

    if ($@) {
        $logger->debug("BUSYGROUP is OK => : $@");
    }
}

sub send {
    my $self = shift;
    my $msg = OpenSRF::Transport::Redis::Message->new(@_);
    
    $msg->body(OpenSRF::Utils::JSON->JSON2perl($msg->body));

    $msg->osrf_xid($logger->get_osrf_xid);
    $msg->from($self->primary_connection->address);

    my $msg_json = $msg->to_json;

    my $recipient = $msg->to;
    my $con = $self->primary_connection;

    if ($recipient =~ /^opensrf:client/o) {
        # Clients may be lurking on remote nodes.
        # Make sure we have a connection to said node.

        # opensrf:client:node_name:...
        my (undef, undef, $node_name) = split(/:/, $recipient);

        my $con = $self->get_connection($node_name);
        if (!$con) {
            $logger->error("Cannot send message to node $node_name: $msg_json");
            return;
        }
    }

    $logger->internal("send(): recipient=$recipient : $msg_json");

    $con->send($recipient, $msg_json);
}

sub process {
    my ($self, $timeout, $for_service) = @_;

    $timeout ||= 0;

    # Redis does not support fractional timeouts.
    $timeout = 1 if ($timeout > 0 && $timeout < 1);

    $timeout = int($timeout);

    if (!$self->connected) {
        # We can't do anything without a primary bus connection.
        # Sleep a short time to avoid die/fork storms, then
        # get outta here.
        $logger->error("We have no primary bus connection");
        sleep 5;
        die "Exiting on lack of primary bus connection";
    }

    my $val = $self->recv($timeout, $for_service);

    return 0 unless $val;

    return OpenSRF::Transport->handler($self->service || 'client', $val);
}

# $timeout=0 means check for data without blocking
# $timeout=-1 means block indefinitely.
sub recv {
    my ($self, $timeout, $for_service) = @_;

    my $dest_stream = $for_service ? $self->{service_address} : undef;

    my $resp = $self->primary_connection->recv($timeout, $dest_stream);

    return undef unless $resp;

    my $msg = OpenSRF::Transport::Redis::Message->new(json => $resp->{msg_json});

    return undef unless $msg;

    $msg->msg_id($resp->{msg_id});

    $logger->internal("recv()'ed thread=" . $msg->thread);

    # The message body is doubly encoded as JSON.
    $msg->body(OpenSRF::Utils::JSON->perl2JSON($msg->body));

    return $msg;
}


sub flush_socket {
    my $self = shift;
    # Remove all messages from our personal stream
    if (my $con = $self->primary_connection) {
        $con->redis->xtrim($con->address, 'MAXLEN', 0);
    }
    return 1;
}

1;


