package OpenSRF::Utils::Conf;
use Net::Domain qw/hostfqdn/;                                                  
use YAML; # sudo apt install libyaml-perl

sub new {
    my ($class, $filename) = @_;
    my $yaml = YAML::LoadFile($filename) || die "Cannot load config: $!\n";

    my $self = bless({
        yaml => $yaml,
        connections => {},          # : HashMap<String, BusConnectionType>,                           
        credentials => {},          # : HashMap<String, BusCredentials>,                              
        domains => [],              # : Vec<BusDomain>,                                                   
        service_groups => {},       # : HashMap<String, Vec<String>>,                              
        log_protect => [],          # : Vec<String>,                                                  
        services => [],             # : Vec<Service>,                                                    
        primary_connection => undef # : Option<BusConnection>,                                 
    }, $class);

    $self->load();

    return $self;
}

# Pull the known config values from the YAML.
# Only settings that control bus connectivity are required.
sub load {
    my $self = shift;
    my $y = $self->yaml;

    if ($y->{'log-protect'}) {
        $self->{log_protect} = $self->yaml->{'log-protect'};
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
                $conf->{workers}->{'max_requests'},
            );

            push(@{$self->{services}}, $service);
        }
    }
}

# Link to the source config file.
# In here you can find anything that's not explicitly unpacked
# by this module.
sub yaml {
    my $self = shift;
    return $self->{yaml};
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
    return bless({
        credentials => $credentials,
        log_level => $log_level,
        log_facility => $log_facility,
        act_facility => $act_facility,
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
    my ($class, $name, $lang, $keepalive, $min_workers, 
        $max_workers, $min_idle_workers, $max_idle_workers, $max_requests) = @_;

    return bless({
        name => $name,
        lang => $lang,
        keepalive => $keepalive,
        min_workers => $min_workers,
        max_workers => $max_workers,
        min_idle_workers => $min_idle_workers,
        max_idle_workers => $max_idle_workers,
        max_requests => $max_requests,
    }, $class);
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
