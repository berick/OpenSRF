package OpenSRF::Utils::Conf;
use Net::Domain qw/hostfqdn/;
use YAML; # sudo apt install libyaml-perl

sub new {
    my $class = shift;
    my $self = bless({});
    $self->reset;
    return $self;
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
    $self->{services} = [];
}

sub load_file {
    my ($self, $filename) = @_;
    $self->reset;
    $self->{source_filename} = $filename;
    $self->{source} = YAML::LoadFile($filename) || die "Cannot load config: $!\n";
    $self->load;
}

# Re-read the configuration
sub reload {
    my $self = shift;
    return $self->load_file($self->{source_filename});
}

sub from_legacy {
    # TODO pull configs OpenSRF::Utils::Config
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

    my $bus = $y->{'message-bus'}; # This one is required.

    my $creds = $bus->{'credentials'};
    while (($key, $value) = each(%$creds)) {
        $self->{credentials}->{$key} =
            OpenSRF::Utils::Conf::BusCredentials->new(
                $value->{username}, $value->{password});
    }

    for my $domain (@{$bus->{domains}}) {

        # Name of service group if set.
        my $services;
        if (my $name = $domain->{'hosted-services'}) {
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

    while (($name, $connection) = each(%{$bus->{connections}})) {

        my $cname = $connection->{credentials};
        my $creds = $self->credentials->{$cname};
        die "No such credentials: $cname\n" unless $creds;

        $self->{connections}->{$name} =
            OpenSRF::Utils::Conf::BusConnectionType->new(
                $creds,
                $connection->{loglevel},
                $connection->{'syslog-facility'},
                $connection->{'actlog-facility'},
            );
    }

    if ($y->{services}) {
        while (($name, $conf) = each(%{$y->{services}})) {
            my $service = OpenSRF::Utils::Conf::Service->new(
                $name,
                $conf->{lang},
                $conf->{keepalive},
                $conf->{workers}->{'min'},
                $conf->{workers}->{'max'},
                $conf->{workers}->{'min-idle'},
                $conf->{workers}->{'max-idle'},
                $conf->{workers}->{'max-requests'},
                $conf->{'app-settings'}
            );

            $service->{source} = $conf;

            push(@{$self->{services}}, $service);
        }
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
sub services {
    my $self = shift;
    return $self->{services};
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
    my ($class, $name, $port, $services) = @_;
    return bless({
        name => $name,
        port => $port,
        services => $services
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
sub services {
    my $self = shift;
    my $s = $self->{services};
    return $s if $s && ref $s eq 'ARRAY' && @$s > 0;
    return undef;
}

package OpenSRF::Utils::Conf::BusConnectionType;

sub new {
    my ($class, $credentials, $log_level, $log_facility, $act_facility) = @_;
    die "BusConnectionType requires credentials\n" unless $credentials;

    return bless({
        credentials => $credentials,
        log_level => $log_level         || 'info',
        log_facility => $log_facility   || 'local0',
        act_facility => $act_facility,  # OK if undef
    }, $class);
}

sub credentials {
    my $self = shift;
    return $self->{credentials};
}
sub log_level {
    my $self = shift;
    return $self->{log_level};
}
sub log_facility {
    my $self = shift;
    return $self->{log_facility};
}
sub act_facility {
    my $self = shift;
    return $self->{act_facility};
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

package OpenSRF::Utils::Conf::Service;

sub new {
    my ($class, $name, $lang, $keepalive, $min_workers, $max_workers,
        $min_idle_workers, $max_idle_workers, $max_requests, $app_settings) = @_;

    die "Service name and lang required.\n" unless $name && $lang;

    return bless({
        name => $name,
        lang => $lang,
        keepalive => $keepalive                 || 6,
        min_workers => $min_workers             || 1,
        max_workers => $max_workers             || 30,
        min_idle_workers => $min_idle_workers   || 1,
        max_idle_workers => $max_idle_workers   || 5,
        max_requests => $max_requests           || 100,
        app_settings => $app_settings           || {},
    }, $class);
}

# Opaque collection of extended app-specific settings.
sub app_settings {
    my $self = shift;
    return $self->{app_settings};
}
# Source config hash.
sub source {
    my $self = shift;
    return $self->{source};
}
sub name {
    my $self = shift;
    return $self->{name};
}
sub lang {
    my $self = shift;
    return $self->{lang};
}
sub keepalive {
    my $self = shift;
    return $self->{keepalive};
}
sub min_workers {
    my $self = shift;
    return $self->{min_workers};
}
sub max_workers {
    my $self = shift;
    return $self->{max_workers};
}
sub min_idle_workers {
    my $self = shift;
    return $self->{min_idle_workers};
}
sub max_idle_workers {
    my $self = shift;
    return $self->{max_idle_workers};
}
sub max_requests {
    my $self = shift;
    return $self->{max_requests};
}

1;
