package OpenSRF::Transport;
use strict; use warnings;
use base 'OpenSRF';
use Time::HiRes qw/time/;
use OpenSRF::AppSession;
use OpenSRF::Utils::JSON;
use OpenSRF::Utils::Logger qw(:level);
use OpenSRF::DomainObject::oilsResponse qw/:status/;
use OpenSRF::EX qw/:try/;
use OpenSRF::Utils::SettingsClient;

#------------------ 
# --- These must be implemented by all Transport subclasses
# -------------------------------------------

=head2 get_peer_client

Returns the name of the package responsible for client communication

=cut

sub get_peer_client { shift()->alert_abstract(); } 

=head2 get_msg_envelope

Returns the name of the package responsible for parsing incoming messages

=cut

sub get_msg_envelope { shift()->alert_abstract(); } 

# -------------------------------------------

our $message_envelope;
my $logger = "OpenSRF::Utils::Logger"; 



=head2 message_envelope( [$envelope] );

Sets the message envelope class that will allow us to extract
information from the messages we receive from the low 
level transport

=cut

sub message_envelope {
    my( $class, $envelope ) = @_;
    if( $envelope ) {
        $message_envelope = $envelope;
        $envelope->use;
        if( $@ ) {
            $logger->error( 
                    "Error loading message_envelope: $envelope -> $@", ERROR);
        }
    }
    return $message_envelope;
}

=head2 handler( $data )

Creates a new MessageWrapper, extracts the remote_id, session_id, and message body
from the message.  Then, creates or retrieves the AppSession object with the session_id and remote_id. 
Finally, creates the message document from the body of the message and calls
the handler method on the message document.

=cut

sub handler {
    my $start_time = time();
    my( $class, $service, $data ) = @_;

    $logger->transport( "Transport handler() received $data", INTERNAL );
    $logger->transport( "Transport handler() received session: " . $data->thread);

    my $remote_id    = $data->from;
    my $sess_id    = $data->thread;
    my $body    = $data->body;
    my $type    = $data->type;

    $logger->set_osrf_xid($data->osrf_xid);

    if (defined($type) and $type eq 'error') {
        throw OpenSRF::EX::Session ("$remote_id IS NOT CONNECTED TO THE NETWORK!!!");

    }

    # See if the app_session already exists.  If so, make 
    # sure the sender hasn't changed if we're a server
    my $app_session = OpenSRF::AppSession->find( $sess_id );
    if( $app_session and $app_session->endpoint == $app_session->SERVER() and
            $app_session->remote_id ne $remote_id ) {

        my $c = OpenSRF::Utils::SettingsClient->new();
        if($c->config_value("apps", $app_session->service, "migratable")) {
            $logger->debug("service is migratable, new client is $remote_id");
        } else {

            $logger->warn("Backend Gone or invalid sender");
            my $res = OpenSRF::DomainObject::oilsBrokenSession->new();
            $res->status( "Backend Gone or invalid sender, Reconnect" );
            $app_session->status( $res );
            return 1;
        }
    } 

    # Retrieve or build the app_session as appropriate (server_build decides which to do)
    $logger->transport( "AppSession $sess_id is valid or does not exist yet", INTERNAL );
    $app_session = OpenSRF::AppSession->server_build( $sess_id, $remote_id, $service );

    if( ! $app_session ) {
        throw OpenSRF::EX::Session ("Transport::handler(): No AppSession object returned from server_build()");
    }


    if (OpenSRF::Application->server_class eq 'client' || 
        OpenSRF::Application->public_service) {

        $logger->internal("Access granted to service: $service");

    } else {
        # Sending any messages to private services without a service
        # key is verboten.

        my $sent_key = $data->service_key || '';
        my $settings = OpenSRF::Utils::SettingsClient->new;
        my $service_key = OpenSRF::Application->private_service_key;

        if (!$service_key) {
            $logger->error(
                "Private serice '$service' has no key; rejecting all messages");
            return 1;
        }

        if ($sent_key eq $service_key) {
            $logger->internal("Correct private service key provided");

        } else {
            $logger->warn("Private service key does not match ".
                "configuration for $service; key sent=$sent_key");

            $app_session->status(
                OpenSRF::DomainObject::oilsMethodException->new( 
                    statusCode => STATUS_FORBIDDEN(),
                    status => "Service $service is private"
                )
            );

            return 1;
        }
    }

    for my $msg (@$body) {

        $logger->internal("Handling message from body " . ref($msg));

        next unless ($msg && UNIVERSAL::isa($msg => 'OpenSRF::DomainObject::oilsMessage'));

        OpenSRF::AppSession->ingress($msg->sender_ingress);

        if( $app_session->endpoint == $app_session->SERVER() ) {

            try {  

                if( ! $msg->handler( $app_session ) ) { return 0; }
                $logger->info(sprintf("Message processing duration: %.3f", (time() - $start_time)));

            } catch Error with {

                my $e = shift;
                my $res = OpenSRF::DomainObject::oilsServerError->new();
                $res->status( $res->status . "\n$e");
                $app_session->status($res) if $res;
                $app_session->kill_me;
                return 0;

            };

        } else { 

            $logger->internal("Client passing message to handler");
            if( ! $msg->handler( $app_session ) ) { return 0; } 
            $logger->debug(sub{return sprintf("Response processing duration: %.3f", (time() - $start_time)) });

        }
    }

    return $app_session;
}

1;
