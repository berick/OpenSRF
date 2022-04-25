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
        $self->{service_key} = $args{service_key} || '';
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

sub service_key {
    my($self, $service_key) = @_;
    $self->{service_key} = $service_key if defined $service_key;
    return $self->{service_key};
}

sub to_json {
    my $self = shift;

    return OpenSRF::Utils::JSON->perl2JSON({
        to => $self->{to},
        from => $self->{from},
        osrf_xid => $self->{osrf_xid},
        service_key => $self->{service_key},
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
