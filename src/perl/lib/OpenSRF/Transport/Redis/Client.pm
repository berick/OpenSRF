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

# Map of bus domains to bus connections.
my %connections;

# There will only be one Client per process, but each client may
# have multiple connections.
my $_singleton;
sub retrieve { return $_singleton; }

sub new {
    my ($class, $connection_type, $service) = @_;

    return $_singleton if $_singleton;

    my $self = {
        service => $service,
        connection_type => $connection_type
    };

    bless($self, $class);

    my $conf = $self->bus_config;

    # Create a connection for our primary domain.
    $self->add_connection($conf->{domain});
    $self->{primary_domain} = $conf->{domain};

    if ($service) {
        # If we're a service, this is where we listen for service-level requests.
        $self->{service_address} = "opensrf:service:$service";
    }

    return $_singleton = $self;
}

sub bus_config {
    my $self = shift;

    my $conf = OpenSRF::Utils::Config->current->as_hash;

    $conf = $conf->{connections} or
        die "No 'connections' block in core configuration\n";

    my $con_type = $self->connection_type;

    $conf = $conf->{$con_type} or
        die "No '$con_type' connection in core configuration\n";

    return $conf;
}

sub connection_type {
    my $self = shift;
    return $self->{connection_type};
}

sub add_connection {
    my ($self, $domain) = @_;

    my $conf = $self->bus_config;

    my $connection = OpenSRF::Transport::Redis::BusConnection->new(
        $domain, 
        $conf->{port}, 
        $conf->{username}, 
        $conf->{password},
        $conf->{max_queue_size}
    );

    $connection->set_address($self->service);
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
        $logger->error("Could not connect to bus on domain: $domain : $@");
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

sub disconnect_all {
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


sub create_service_stream {
    my $self = shift;

    eval { 
        # This gets mad when a stream / group already exists, 
        # but Workers share a stream/group name when receiving 
        # service-level requests

        $self->primary_connection->redis->xgroup(   
            'create',
            $self->service_address, # stream name
            $self->service_address, # group name
            '$',            # only receive new messages
            'mkstream'      # create this stream if it's not there.
        );
    };

    if ($@) {
        $logger->debug("create_service_stream returned : $@");
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
        # Clients may be lurking on remote domains.
        # Make sure we have a connection to said domain.

        # opensrf:client:domain:...
        my (undef, undef, $domain) = split(/:/, $recipient);

        my $con = $self->get_connection($domain);
        if (!$con) {
            $logger->error("Cannot send message to domain $domain: $msg_json");
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

    return OpenSRF::Transport->handler($self->service, $val);
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

    $logger->internal("recv() thread=" . $msg->thread);

    # The message body is doubly encoded as JSON.
    $msg->body(OpenSRF::Utils::JSON->perl2JSON($msg->body));

    return $msg;
}


sub flush_socket {
    my $self = shift;
    # Remove all messages from the stream
    $self->redis->xtrim($self->stream_name, 'MAXLEN', 0);
    return 1;
}

1;


