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

    $conf = $conf->{$self->connection_type} or
        die "No '$connection_type' connection in core configuration\n";

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

    # TODO this could be a service address or a worker/client address.
    my $recipient = $msg->to;
    my (undef, 

    $logger->internal("send(): to=" . $msg->to . " : $msg_json");

    $self->redis->xadd(
        $msg->to,                   # recipient == stream name
        'NOMKSTREAM',
        'MAXLEN', 
        '~',                        # maxlen-ish
        $self->max_queue_size,
        '*',                        # let Redis generate the ID
        'message',                  # gotta call it something 
        $msg_json
    );
}

sub process {
    my ($self, $timeout) = @_;

    $timeout ||= 0;

    # Redis does not support fractional timeouts.
    $timeout = 1 if ($timeout > 0 && $timeout < 1);

    $timeout = int($timeout);

    unless ($self->redis) {
        throw OpenSRF::EX::JabberDisconnected 
            ("This Redis instance is no longer connected to the server ");
    }

    my $value = $self->recv($timeout);

    return 0 unless $value;

    return OpenSRF::Transport->handler($self->service, $val);
}

# $timeout=0 means check for data without blocking
# $timeout=-1 means block indefinitely.
sub recv {
    my ($self, $timeout) = @_;

    $logger->debug("server: watching for content at " . $self->stream_name);

    my @block;
    if ($timeout) {
        # 0 means block indefinitely in Redis
        $timeout = 0 if $timeout == -1;
        $timeout *= 1000; # milliseconds
        @block = (BLOCK => $timeout);
    }

    my $packet = $self->redis->xreadgroup(
        GROUP => $self->stream_name,
        $self->consumer_name,
        @block,
        COUNT => 1,
        STREAMS => $self->stream_name,
        '>' # new messages only
    );      

    # Timed out waiting for data.
    return undef unless defined $packet;

    # TODO make this more self-documenting.  also, too brittle?
    my $container = $packet->[0]->[1]->[0];
    my $msg_id = $container->[0];
    my $json = $container->[1]->[1];

    $logger->internal("recv() $json");

    # TODO putting this here for now -- it may live somewhere else.
    # Ideally this could happen out of band.
    # Note if we don't ACK utnil after successfully processing each
    # message, a malformed message will stay in the pending list.
    # Consider options.
    $self->redis->xack($self->stream_name, $self->stream_name, $msg_id);

    my $msg = OpenSRF::Transport::Redis::Message->new(json => $json);
    $msg->msg_id($msg_id);

    return undef unless $msg;

    $logger->internal("recv() thread=" . $msg->thread);

    # The message body is doubly encoded as JSON to, among other things,
    # support message chunking.
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


