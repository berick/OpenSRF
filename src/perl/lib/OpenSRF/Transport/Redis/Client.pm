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

# Map of bus domain names to bus connections.
my %connections;

# There will only be one Client per process, but each client may
# have multiple connections.
my $_singleton;
sub retrieve { return $_singleton; }

sub new {
    my ($class, $service, $no_cache) = @_;

    return $_singleton if $_singleton && !$no_cache;

    my $self = {service => $service};

    bless($self, $class);

    my $conf = OpenSRF::Utils::Config->current;
    my $domain = $conf->bootstrap->domain;

    # Create a connection for our primary node.
    $self->add_connection($domain);
    $self->{primary_domain} = $domain;

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
    my ($self, $domain) = @_;

    my $conf = OpenSRF::Utils::Config->current;

    my $username = $conf->bootstrap->username;
    my $password = $conf->bootstrap->passwd;
    my $port = $conf->bootstrap->port;
    my $max_queue = 1024; # TODO

    # Assumes other connection parameters are the same across
    # Redis instances, apart from the hostname.
    my $connection = OpenSRF::Transport::Redis::BusConnection->new(
        $domain, $port, $username, $password, $max_queue
    );

    $connection->set_address();
    $connections{$domain} = $connection;
    
    $connection->connect;

    return $connection;
}

sub get_connection {
    my ($self, $domain) = @_;

    my $con = $connections{$domain};

    return $con if $con;

    eval { $con = $self->add_connection($domain) };

    if ($@) {
        $logger->error("Could not connect to bus on node: $domain : $@");
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

sub primary_domain {
    my $self = shift;
    return $self->{primary_domain};
}

sub primary_connection {
    my $self = shift;
    return $connections{$self->primary_domain};
}

sub disconnect {
    my ($self, $domain) = @_;

    for my $domain (keys %connections) {
        my $con = $connections{$domain};
        $con->disconnect($self->primary_domain eq $domain);
        delete $connections{$domain};
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

        # opensrf:client:domain:...
        my (undef, undef, $domain) = split(/:/, $recipient);

        my $con = $self->get_connection($domain);
        if (!$con) {
            $logger->error("Cannot send message to node $domain: $msg_json");
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


