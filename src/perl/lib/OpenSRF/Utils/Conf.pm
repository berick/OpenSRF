package OpenSRF::Utils::Conf;
use Net::Domain qw/hostfqdn/;
use YAML; # sudo apt install libyaml-perl

my $singleton;

sub new {
    my $class = shift;
    my $self = $singleton = bless({});
    $self->reset;
    return $self;
}

sub current {
    $singleton
}

# Does not reset our primary_connection
sub reset {
    my $self = shift;
    $self->{source} = undef;
    $self->{connections} = {};
    $self->{credentials} = {};
    $self->{domains} = [];
    $self->{service_groups} = {};
    $self->{log_protect} = [];
}

sub load_file {
    my ($self, $filename) = @_;
    $self->reset;
    $self->{source_filename} = $filename;
    $self->{source} = YAML::LoadFile($filename) || die "Cannot load config: $!\n";
    $self->load;
}

# This is handy
sub hostname {
    hostfqdn();
}

# Re-read the configuration
sub reload {
    my $self = shift;
    return $self->load_file($self->{source_filename});
}

# Pull the known config values from the YAML.
# Only settings that control bus connectivity are required.
sub load {
    my $self = shift;
    my $y = $self->source;

    if ($y->{'log-protect'}) {
        $self->{log_protect} = $y->{'log-protect'};
    }

    if ($y->{'service-groups'}) {
        $self->{service_groups} = $y->{'service-groups'};
    }

    my $bus = $y->{'message-bus'} || 
        die "message-bus: configuration required\n";


    my $creds = $bus->{'credentials'};
    while (($key, $value) = each(%$creds)) {
        $self->{credentials}->{$key} =
            OpenSRF::Utils::Conf::BusCredentials->new(
                $value->{username}, $value->{password});
    }

    for my $domain (@{$bus->{domains}}) {

        # Name of service group if set.
        my $services;
        if (my $name = $domain->{'allowed-services'}) {
            $services = $self->service_groups->{$name};
            die "No such service group: $name\n" unless $services;
        }

        push(@{$self->{domains}},
            OpenSRF::Utils::Conf::BusDomain->new(
                $domain->{name},
                $domain->{port} || 6379,
                $services
            )
        );
    }

    my $log_defaults = $bus->{'log-defaults'} || {};

    while (($name, $connection) = each(%{$bus->{connections}})) {

        my $cname = $connection->{credentials};
        my $creds = $self->credentials->{$cname};
        die "No such credentials: $cname\n" unless $creds;

        # pull the value from the connection or the default logging configs.
        my $lv = sub { my $t = shift; $connection->{$t} || $log_defaults->{$t} };

        $self->{connections}->{$name} =
            OpenSRF::Utils::Conf::BusConnectionType->new(
                $creds,
                log_level             => $lv->('log-level'),
                log_file              => $lv->('log-file'),
                syslog_facility       => $lv->('syslog-facility'),
                activity_log_file     => $lv->('activity-log-file'),
                activity_log_facility => $lv->('activity-log-facility')
            );

        $self->{connections}->{$name}->{log_length} = $connection->{'log-length'};
        $self->{connections}->{$name}->{log_tag} = $connection->{'log-tag'};
    }
}

# Link to the source config file.
# In here you can find anything that's not explicitly unpacked
# by this module.
sub source {
    my $self = shift;
    return $self->{source};
}
sub connections {
    my $self = shift;
    return $self->{connections};
}
sub credentials {
    my $self = shift;
    return $self->{credentials};
}
sub domains {
    my $self = shift;
    return $self->{domains};
}
# Named lists of service names
sub service_groups {
    my $self = shift;
    return $self->{service_groups};
}
sub log_protect {
    my $self = shift;
    return $self->{log_protect};
}
sub primary_connection {
    my $self = shift;
    return $self->{primary_connection};
}

sub set_primary_connection {
    my ($self, $domain, $ctype) = @_;
    my $ct = $self->connections->{$ctype};
    my ($dm) = grep {$_->name eq $domain} @{$self->domains};

    die "No such connection type: $ctyp\n" unless $ct;
    die "No such domain: $domain\n" unless $dm;

    $self->{primary_connection} =
        OpenSRF::Utils::Conf::BusConnection->new($dm, $ct);
}


package OpenSRF::Utils::Conf::BusCredentials;

sub new {
    my ($class, $username, $password) = @_;
    return bless({
        username => $username,
        password => $password
    }, $class);
}

sub username {
    my $self = shift;
    return $self->{username};
}

sub password {
    my $self = shift;
    return $self->{password};
}

package OpenSRF::Utils::Conf::BusDomain;

sub new {
    my ($class, $name, $port, $allowed_services) = @_;
    return bless({
        name => $name,
        port => $port,
        allowed_services => $allowed_services
    }, $class);
}

sub name {
    my $self = shift;
    return $self->{name};
}

sub port {
    my $self = shift;
    return $self->{port};
}

# Returns an array ref if this domain hosts a specific
# set of services.  Returns undef otherwise.
sub allowed_services {
    my $self = shift;
    my $s = $self->{allowed_services};
    return $s if $s && ref $s eq 'ARRAY' && @$s > 0;
    return undef;
}

package OpenSRF::Utils::Conf::BusConnectionType;

sub new {
    my ($class, $credentials, %args) = @_;

    die "BusConnectionType requires credentials\n" unless $credentials;

    return bless({credentials => $credentials, %args}, $class);
}

sub max_queue_length {
    my $self = shift;
    return ($self->{max_queue_length} || 0) || 1000;
}
sub credentials {
    my $self = shift;
    return $self->{credentials};
}
sub log_level {
    my $self = shift;
    return $self->{log_level};
}
sub log_file {
    my $self = shift;
    return $self->{log_file};
}
sub syslog_facility {
    my $self = shift;
    return $self->{syslog_facility};
}
sub activity_log_file {
    my $self = shift;
    return $self->{activity_log_file};
}
sub activity_log_facility {
    my $self = shift;
    return $self->{activity_log_facility};
}
sub log_length {
    my $self = shift;
    return $self->{log_length};
}
sub log_tag {
    my $self = shift;
    return $self->{log_tag};
}

package OpenSRF::Utils::Conf::BusConnection;

sub new {
    my ($class, $domain, $connection_type) = @_;
    return bless({
        domain => $domain,
        connection_type => $connection_type
    }, $class);
}

# BusDomain object
sub domain {
    my $self = shift;
    return $self->{domain};
}

# BusConnectionType object
sub connection_type {
    my $self = shift;
    return $self->{connection_type};
}

1;
