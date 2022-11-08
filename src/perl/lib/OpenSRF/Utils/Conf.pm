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
        my $private_conf = $domain->{private_node};
        my $public_conf = $domain->{public_node};

        my $private_node = OpenSRF::Utils::Conf::Node->new(
            $private_conf->{name},
            $private_conf->{port},
            $self->service_group_to_services($private_conf->{allowed_services})
        );

        my $public_node = OpenSRF::Utils::Conf::Node->new(
            $public_conf->{name},
            $public_conf->{port},
            $self->service_group_to_services($public_conf->{allowed_services})
        );

        $self->{domains}->{$name} = 
            OpenSRF::Utils::Conf::Domain->new(
            $name, $private_node, $public_node, $domain->{hosts});
    }

    my $log_defaults = $conf->{log_defaults} || {};

    while (my ($name, $connection) = each(%{$conf->{connections}})) {

        my $node_type = $connection->{node_type}; # public vs. private
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
                $node_type, $creds, %params
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
sub get_node {
    my ($self, $node_name) = @_;
    for my $domain (values(%{$self->domains})) {
        return $domain->private_node if $domain->private_node->name eq $node_name;
        return $domain->public_node if $domain->public_node->name eq $node_name;
    }
    die "No such node: $node_name\n";
}

# Sets the primary connection to the provided type and domain.
# The physical node is determimed by the connection type.
# Returns the newly applied Connection
sub set_primary_connection {
    my ($self, $domain_name, $connection_type) = @_;

    my $ctype = $self->connections->{$connection_type} ||
        die "No such connection type: $connection_type\n";

    my $domain = $self->domains->{$domain_name}
        || "No configuration for domain: $domain_name\n";

    # Get the node where this connection type wants to connect.

    my $node = ($ctype->node_type =~ /public/i) ? $domain->public_node : $domain->private_node;

    $self->{primary_connection} =
        OpenSRF::Utils::Conf::Connection->new($node, $ctype);

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

package OpenSRF::Utils::Conf::Node;

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
    my $s = $self->{allowed_services};
    return $s if $s && ref $s eq 'ARRAY' && @$s > 0;
    return undef;
}
sub max_queue_length {
    my $self = shift;
    return ($self->{max_queue_length} || 0) || 1000;
}

package OpenSRF::Utils::Conf::Domain;

sub new {
    my ($class, $name, $private_node, $public_node, $hosts) = @_;
    return bless({
        name => $name,
        private_node => $private_node,
        public_node => $public_node,
        hosts => $hosts
    }, $class);
}
sub hosts {
    my $self = shift;
    return $self->{hosts};
}
sub private_node {
    my $self = shift;
    return $self->{private_node};
}
sub public_node {
    my $self = shift;
    return $self->{public_node};
}

package OpenSRF::Utils::Conf::ConnectionType;

sub new {
    my ($class, $node_type, $credentials, %args) = @_;

    die "ConnectionType requires domain and credentials\n" 
        unless $node_type && $credentials;

    die "Invalid node_type type: $node_type"
        unless $node_type =~ /^(public|private)$/i;

    return bless({
        credentials => $credentials, node_type => $node_type, %args}, $class);
}

sub node_type {
    my $self = shift;
    return $self->{node_type};
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
    my ($class, $node, $connection_type) = @_;
    return bless({
        node => $node,
        connection_type => $connection_type
    }, $class);
}

# Node object
sub node {
    my $self = shift;
    return $self->{node};
}

# ConnectionType object
sub connection_type {
    my $self = shift;
    return $self->{connection_type};
}

1;
