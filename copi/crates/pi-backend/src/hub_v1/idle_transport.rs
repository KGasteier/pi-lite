//! ureq 3.4 hat nur Gesamtbudgets. Leerlauf deshalb am TCP-Transport begrenzen,
//! VOR TLS: jeder Socket-Read/-Write erhält ein frisches Budget. TLS-Verifikation
//! und HTTP-Framing bleiben vollständig bei ureq. Keine Hintergrund-Reader.
use std::time::Duration;
use ureq::unversioned::{
    resolver::DefaultResolver,
    transport::{
        Buffers, ConnectionDetails, Connector, NextTimeout, RustlsConnector, TcpConnector,
        Transport,
    },
};

#[derive(Debug)]
struct IdleConnector(Duration);
#[derive(Debug)]
struct IdleTransport<T> {
    inner: T,
    idle: Duration,
}
impl<T: Transport> Connector<T> for IdleConnector {
    type Out = IdleTransport<T>;
    fn connect(
        &self,
        _: &ConnectionDetails,
        chained: Option<T>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        Ok(chained.map(|inner| IdleTransport {
            inner,
            idle: self.0,
        }))
    }
}
impl<T: Transport> IdleTransport<T> {
    fn bounded(&self, timeout: NextTimeout) -> NextTimeout {
        // Ein kürzeres Phasenbudget (z. B. TLS-Verbindungsaufbau) nie verlängern.
        if timeout.after.is_not_happening() || *timeout.after > self.idle {
            NextTimeout {
                after: self.idle.into(),
                reason: ureq::Timeout::RecvBody,
            }
        } else {
            timeout
        }
    }
}
impl<T: Transport> Transport for IdleTransport<T> {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.inner.buffers()
    }
    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.inner.transmit_output(amount, self.bounded(timeout))
    }
    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        self.inner.await_input(self.bounded(timeout))
    }
    fn is_open(&mut self) -> bool {
        self.inner.is_open()
    }
    fn is_tls(&self) -> bool {
        self.inner.is_tls()
    }
}

pub(super) fn agent(idle: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .proxy(None)
        .timeout_connect(Some(Duration::from_secs(10)))
        .build();
    ureq::Agent::with_parts(
        config,
        TcpConnector::default()
            .chain(IdleConnector(idle))
            .chain(RustlsConnector::default()),
        DefaultResolver::default(),
    )
}
