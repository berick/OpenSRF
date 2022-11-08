package OpenSRF::Utils::Conf;
use strict;
use warnings;
use Net::Domain qw/hostfqdn hostdomain/;
use YAML; # sudo apt install libyaml-perl

my $singleton;

sub current {
    $singleton
}

sub from_file {
    my ($class, $filename) = @_;
    my $self = $singleton = bless({});
    $self->load_file($filename);
    return $self;
}

# Does not reset our primary_connection
sub reset {
    my $self = shift;
    $self->{source} = undef;
    $self->{connections} = {};
    $self->{credentials} = {};
    $self->{domains} = {};
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
    $ENV{OSRF_HOSTNAME} || hostfqdn();
}

sub domain {
    $ENV{OSRF_DOMAIN} || hostdomain();
}

sub settings_config {
    my $self = shift;
    return $self->{settings_config};
}

# Re-read the configuration
sub reload {
    my $self = shift;
    return $self->load_file($self->{source_filename});
}

sub service_group_to_services {
    my ($self, $group) = @_;
    return undef unless $group;

    my $services = $self->service_groups->{$group} ||
        die "No such service group: $group\n";
    return $services;
}

# Pull the known config values from the YAML.
# Only settings that control bus connectivity are required.
sub load {
    my $self = shift;
    my $conf = $self->source;

    $self->{settings_config} = $conf->{settings_config};
    $self->{log_protect} = $conf->{log_protect};
    $self->{service_groups} = $conf->{service_groups};

    my $creds = $conf->{credentials};
    while (my ($key, $value) = each(%$creds)) {
        $self->{credentials}->{$key} =
            OpenSRF::Utils::Conf::Credentials->new(
                $value->{username}, $value->{password});
    }

    for my $domain (@{$conf->{domains}}) {

        my $name = $domain->{name};
        my $private_conf = $domain->{private};
        my $public_conf = $domain->{public};

        my $private = OpenSRF::Utils::Conf::SubDomain->new(
            $private_conf->{name},
            $private_conf->{port},
            $self->service_group_to_services($private_conf->{allowed_services})
        );

        my $public = OpenSRF::Utils::Conf::SubDomain->new(
            $public_conf->{name},
            $public_conf->{port},
            $self->service_group_to_services($public_conf->{allowed_services})
        );

        $self->{domains}->{$name} = 
            OpenSRF::Utils::Conf::Domain->new(
            $name, $private, $public, $domain->{hosts});
    }

    my $log_defaults = $conf->{log_defaults} || {};

    while (my ($name, $connection) = each(%{$conf->{connections}})) {

        my $subdomain = $connection->{subdomain}; # public vs. private
        my $cname = $connection->{credentials};
        my $creds = $self->credentials->{$cname};

        die "No such credentials: $cname\n" unless $creds;

        my %params;
        for my $key (qw/
            log_level
            log_file
            syslog_facility
            activity_log_file
            activity_log_facility
            log_length
            log_tag
        /) { $params{$key} = $connection->{$key} || $log_defaults->{$key}; }

        $self->{connections}->{$name} =
            OpenSRF::Utils::Conf::ConnectionType->new(
                $subdomain, $creds, %params
            );
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
sub get_subdomain {
    my ($self, $subdomain) = @_;
    for my $domain (values(%{$self->domains})) {
        return $domain->private if $domain->private->name eq $subdomain;
        return $domain->public if $domain->public->name eq $subdomain;
    }
    die "No such subdomain: $subdomain\n";
}

# Returns the newly applied Connection
sub set_primary_connection {
    my ($self, $domain_name, $connection_type) = @_;

    my $ctype = $self->connections->{$connection_type} ||
        die "No such connection type: $connection_type\n";

    my $domain = $self->domains->{$domain_name}
        || "No configuration for domain: $domain_name\n";

    # Get the subdomain where this connection type wants to connect.

    my $subdomain = 
        $ctype->subdomain eq 'public' ? $domain->public : $domain->private;

    $self->{primary_connection} =
        OpenSRF::Utils::Conf::Connection->new($subdomain, $ctype);

    return $self->{primary_connection};
}


package OpenSRF::Utils::Conf::Credentials;

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

package OpenSRF::Utils::Conf::SubDomain;

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
sub allowed_services {
    my $self = shift;
    return $self->{allowed_services};
}
sub max_queue_length {
    my $self = shift;
    return ($self->{max_queue_length} || 0) || 1000;
}

package OpenSRF::Utils::Conf::Domain;

sub new {
    my ($class, $name, $private_domain, $public_domain, $hosts) = @_;
    return bless({
        name => $name,
        private => $private_domain,
        public => $public_domain,
        hosts => $hosts
    }, $class);
}

sub hosts {
    my $self = shift;
    return $self->{hosts};
}
sub private {
    my $self = shift;
    return $self->{private};
}
sub public {
    my $self = shift;
    return $self->{public};
}


# Returns an array ref if this domain hosts a specific
# set of services.  Returns undef otherwise.
sub allowed_services {
    my $self = shift;
    my $s = $self->{allowed_services};
    return $s if $s && ref $s eq 'ARRAY' && @$s > 0;
    return undef;
}

package OpenSRF::Utils::Conf::ConnectionType;

sub new {
    my ($class, $subdomain, $credentials, %args) = @_;

    die "ConnectionType requires domain and credentials\n" 
        unless $subdomain && $credentials;

    die "Invalid subdomain type: $subdomain"
        unless $subdomain =~ /^(public|private)$/;

    return bless({
        credentials => $credentials, subdomain => $subdomain, %args}, $class);
}

# SubDomain object 
sub subdomain {
    my $self = shift;
    return $self->{subdomain};
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

package OpenSRF::Utils::Conf::Connection;

sub new {
    my ($class, $subdomain, $connection_type) = @_;
    return bless({
        subdomain => $subdomain,
        connection_type => $connection_type
    }, $class);
}

# SubDomain object
sub subdomain {
    my $self = shift;
    return $self->{subdomain};
}

# ConnectionType object
sub connection_type {
    my $self = shift;
    return $self->{connection_type};
}

1;
