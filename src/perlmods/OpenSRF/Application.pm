package OpenSRF::Application;
use base qw/OpenSRF/;
use OpenSRF::AppSession;
use OpenSRF::DomainObject::oilsMethod;
use OpenSRF::DomainObject::oilsResponse qw/:status/;
use OpenSRF::Utils::Logger qw/:level/;
use Time::HiRes qw/time/;
use vars qw/$_app $log/;
use OpenSRF::EX qw/:try/;
use strict;
use warnings;

$log = 'OpenSRF::Utils::Logger';

our $in_request = 0;
our @pending_requests;

sub application_implementation {
	my $self = shift;
	my $app = shift;

	if (defined $app) {
		$_app = $app;
		eval "use $_app;";
		if( $@ ) {
			$log->error( "Error loading application_implementation: $app -> $@", ERROR);
		}

	}

	return $_app;
}

sub handler {
	my ($self, $session, $app_msg) = @_;

	$log->debug( "In Application::handler()", DEBUG );

	my $app = $self->application_implementation;

	if( $app ) {
		$log->debug( "Application is $app", DEBUG);
	}
	$log->debug( "Message is ".$app_msg->toString(1), INTERNAL);


	if ($session->last_message_type eq 'REQUEST') {
		$log->debug( "We got a REQUEST: ". $app_msg->method, INFO );

		my $method_name = $app_msg->method;
		$log->debug( " * Looking up $method_name inside $app", DEBUG);

		my $method_proto = $session->last_message_protocol;
		$log->debug( " * Method API Level [$method_proto]", DEBUG);

		my $coderef = $app->method_lookup( $method_name, $method_proto );

		unless ($coderef) {
			$session->status( OpenSRF::DomainObject::oilsMethodException->new() );
			return 1;
		}

		#if ( $session->client_auth->username || $session->client_auth->userid ) {
		#	unless ( $coderef->is_action ) {
		#		$session->status(
		#			OpenSRF::DomainObject::oilsMethodException->new(
		#					statusCode => STATUS_NOTALLOWED(),
		#					status => "User cannot use [$method_name]" ) );
		#		return 1;
		#	}
		#}
			

		$log->debug( " (we got coderef $coderef", DEBUG);

		unless ($session->continue_request) {
			$session->status(
				OpenSRF::DomainObject::oilsConnectStatus->new(
						statusCode => STATUS_REDIRECTED(),
						status => 'Disconnect on max requests' ) );
			$session->kill_me;
			return 1;
		}

		if (ref $coderef) {
			my @args = $app_msg->params;
			my $appreq = OpenSRF::AppRequest->new( $session );

			$log->debug( "in_request = $in_request : [" . $appreq->threadTrace."]", DEBUG );
			if( $in_request ) {
				$log->debug( "Pushing onto pending requests: " . $appreq->threadTrace, DEBUG );
				push @pending_requests, [ $appreq, \@args, $coderef ]; 
				return 1;
			}


			$in_request++;

			$log->debug( "Executing coderef for {$method_name -> ".join(', ', @args)."}", INTERNAL );

			my $resp;
			try {
				my $start = time();
				$resp = $coderef->run( $appreq, @args); 
				my $time = sprintf '%.3f', time() - $start;
				$log->debug( "Method duration for {$method_name -> ".join(', ', @args)."}:  ". $time, DEBUG );
				if( ref( $resp ) ) {
					$log->debug( "Calling respond_complete: ". $resp->toString(), INTERNAL );
					$appreq->respond_complete( $resp );
				} else {
				        $appreq->status( OpenSRF::DomainObject::oilsConnectStatus->new(
								statusCode => STATUS_COMPLETE(),
								status => 'Request Complete' ) );
				}
			} catch Error with {
				my $e = shift;
				$e = $e->{-text} || $e->message if (ref $e);
				my $sess_id = $session->session_id;
				$session->status(
					OpenSRF::DomainObject::oilsMethodException->new(
							statusCode	=> STATUS_INTERNALSERVERERROR(),
							status		=> " *** Call to [$method_name] failed for session ".
									   "[$sess_id], thread trace [".$appreq->threadTrace."]:\n".$e
					)
				);
			};



			# ----------------------------------------------


			# XXX may need this later
			# $_->[1] = 1 for (@OpenSRF::AppSession::_CLIENT_CACHE);

			$in_request--;

			$log->debug( "Pending Requests: " . scalar(@pending_requests), INTERNAL );

			# cycle through queued requests
			while( my $aref = shift @pending_requests ) {
				$in_request++;
				my $resp;
				try {
					my $start = time;
					my $response = $aref->[2]->run( $aref->[0], @{$aref->[1]} );
					my $time = sprintf '%.3f', time - $start;
					$log->debug( "Method duration for {[".$aref->[2]->name." -> ".join(', ',@{$aref->[1]}).'}:  '.$time, DEBUG );

					$appreq = $aref->[0];	
					if( ref( $response ) ) {
						$log->debug( "Calling respond_complete: ". $response->toString(), INTERNAL );
						$appreq->respond_complete( $response );
					} else {
					        $appreq->status( OpenSRF::DomainObject::oilsConnectStatus->new(
									statusCode => STATUS_COMPLETE(),
									status => 'Request Complete' ) );
					}
					$log->debug( "Executed: " . $appreq->threadTrace, DEBUG );
				} catch Error with {
					my $e = shift;
					$session->status(
						OpenSRF::DomainObject::oilsMethodException->new(
								statusCode => STATUS_INTERNALSERVERERROR(),
								status => "Call to [".$aref->[2]->name."] faild:  ".$e->{-text}
						)
					);
				};
				$in_request--;
			}

			return 1;
		} 
		my $res = OpenSRF::DomainObject::oilsMethodException->new;
		$session->send('ERROR', $res);
		$session->kill_me;
		return 1;

	} else {
		$log->debug( "Pushing ". $app_msg->toString ." onto queue", INTERNAL );
		$session->push_queue([ $app_msg, $session->last_threadTrace ]);
	}

	$session->last_message_type('');
	$session->last_message_protocol('');

	return 1;
}

sub method_lookup {
	my $self = shift;
	my $method = shift;
	my $proto = shift;

	my $class = ref($self) || $self;

	$log->debug("Looking up [$method] in [$self]", INTERNAL);
	
	my $obj = bless {} => $self;
	if (my $coderef = $self->can("${method}_${proto}")) {
		$$obj{code} = $coderef;
		$$obj{name} = "${method}_${proto}";
		return $obj;
	}
	return undef;
}

sub run {
	my $self = shift;
	$self->{code}->(@_);
}

sub is_action {
	my $self = shift;
	if (my $can = $self->can($self->{name} . '_action')) {
		return $can->();
	}
	return 0;
}


1;
