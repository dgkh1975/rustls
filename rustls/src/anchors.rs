use crate::key;
#[cfg(feature = "logging")]
use crate::log::{debug, trace};
use crate::msgs::handshake::{DistinguishedName, DistinguishedNames};
use crate::x509;

/// This is like a `webpki::TrustAnchor`, except it owns
/// rather than borrows its memory.  That prevents lifetimes
/// leaking up the object tree.
#[derive(Debug, Clone)]
pub struct OwnedTrustAnchor {
    subject: Vec<u8>,
    spki: Vec<u8>,
    name_constraints: Option<Vec<u8>>,
}

impl OwnedTrustAnchor {
    /// Get a `webpki::TrustAnchor` by borrowing the owned elements.
    pub(crate) fn to_trust_anchor(&self) -> webpki::TrustAnchor {
        webpki::TrustAnchor {
            subject: &self.subject,
            spki: &self.spki,
            name_constraints: self.name_constraints.as_deref(),
        }
    }
}

impl From<&webpki::TrustAnchor<'_>> for OwnedTrustAnchor {
    fn from(t: &webpki::TrustAnchor) -> Self {
        Self {
            subject: t.subject.to_vec(),
            spki: t.spki.to_vec(),
            name_constraints: t.name_constraints.map(|x| x.to_vec()),
        }
    }
}

/// A container for root certificates able to provide a root-of-trust
/// for connection authentication.
#[derive(Debug, Clone)]
pub struct RootCertStore {
    /// The list of roots.
    pub roots: Vec<OwnedTrustAnchor>,
}

impl RootCertStore {
    /// Make a new, empty `RootCertStore`.
    pub fn empty() -> Self {
        Self { roots: Vec::new() }
    }

    /// Return true if there are no certificates.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Say how many certificates are in the container.
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    /// Return the Subject Names for certificates in the container.
    pub fn subjects(&self) -> DistinguishedNames {
        let mut r = DistinguishedNames::new();

        for ota in &self.roots {
            let mut name = Vec::new();
            name.extend_from_slice(&ota.subject);
            x509::wrap_in_sequence(&mut name);
            r.push(DistinguishedName::new(name));
        }

        r
    }

    /// Add a single DER-encoded certificate to the store.
    pub fn add(&mut self, der: &key::Certificate) -> Result<(), webpki::Error> {
        let ta = webpki::TrustAnchor::try_from_cert_der(&der.0)?;
        let ota = OwnedTrustAnchor::from(&ta);
        self.roots.push(ota);
        Ok(())
    }

    /// Adds all the given TrustAnchors `anchors`.  This does not
    /// fail.
    pub fn add_server_trust_anchors<'a, I, T: 'a>(&mut self, iter: I)
    where
        I: IntoIterator<Item = &'a T> + 'a,
        &'a T: Into<OwnedTrustAnchor>,
    {
        self.roots
            .extend(iter.into_iter().map(|ta| ta.into()));
    }

    /// Parse the given DER-encoded certificates and add all that can be parsed
    /// in a best-effort fashion.
    ///
    /// This is because large collections of root certificates often
    /// include ancient or syntactically invalid certificates.
    ///
    /// Returns the number of certificates added, and the number that were ignored.
    pub fn add_parsable_certificates(&mut self, der_certs: &[Vec<u8>]) -> (usize, usize) {
        let mut valid_count = 0;
        let mut invalid_count = 0;

        for der_cert in der_certs {
            #[cfg_attr(not(feature = "logging"), allow(unused_variables))]
            match self.add(&key::Certificate(der_cert.clone())) {
                Ok(_) => valid_count += 1,
                Err(err) => {
                    trace!("invalid cert der {:?}", der_cert);
                    debug!("certificate parsing failed: {:?}", err);
                    invalid_count += 1
                }
            }
        }

        debug!(
            "add_parsable_certificates processed {} valid and {} invalid certs",
            valid_count, invalid_count
        );

        (valid_count, invalid_count)
    }
}
