package OpenSRF::Transport::Redis::Message;
use strict; use warnings;
use OpenSRF::Utils::Logger qw/$logger/;
use OpenSRF::Utils::JSON;
use OpenSRF::EX qw/:try/;
use strict; use warnings;

sub new {
    my ($class, %args) = @_;
    my $self = bless({}, $class);

    if ($args{json}) {
        $self->from_json($args{json});

    } else {
        $self->{to} = $args{to} || '';
        $self->{from} = $args{from} || '';
        $self->{thread} = $args{thread} || '';
        $self->{body} = $args{body} || '';
        $self->{osrf_xid} = $args{osrf_xid} || '';
        $self->{msg_id} = $args{msg_id} || '';
    }

    return $self;
}

sub to {
    my($self, $to) = @_;
    $self->{to} = $to if defined $to;
    return $self->{to};
}
sub from {
    my($self, $from) = @_;
    $self->{from} = $from if defined $from;
    return $self->{from};
}
sub thread {
    my($self, $thread) = @_;
    $self->{thread} = $thread if defined $thread;
    return $self->{thread};
}
sub body {
    my($self, $body) = @_;
    $self->{body} = $body if defined $body;
    return $self->{body};
}

sub status {
    my($self, $status) = @_;
    $self->{status} = $status if defined $status;
    return $self->{status};
}
sub type {
    my($self, $type) = @_;
    $self->{type} = $type if defined $type;
    return $self->{type};
}

sub err_type {}
sub err_code {}

sub osrf_xid {
    my($self, $osrf_xid) = @_;
    $self->{osrf_xid} = $osrf_xid if defined $osrf_xid;
    return $self->{osrf_xid};
}

sub msg_id {
    my($self, $msg_id) = @_;
    $self->{msg_id} = $msg_id if defined $msg_id;
    return $self->{msg_id};
}

sub to_json {
    my $self = shift;

    # No nead to encode the msg_id in outbound messages since the ID
    # won't exist yet.
    return OpenSRF::Utils::JSON->perl2JSON({
        to => $self->{to},
        from => $self->{from},
        osrf_xid => $self->{osrf_xid},
        thread => $self->{thread},
        body => $self->{body}
    });
}

sub from_json {
    my $self = shift;
    my $json = shift;
    my $hash;

    eval { $hash = OpenSRF::Utils::JSON->JSON2perl($json); };

    if ($@) {
        $logger->error("Redis::Message received invalid JSON: $@ : $json");
        return undef;
    }

    $self->{$_} = $hash->{$_} for keys %$hash;
}

1;
