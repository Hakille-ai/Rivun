//! Serde helper modules for 64-byte Ed25519 signatures.

pub mod signature_bytes {
    use serde::{Deserializer, Serializer, de::Error, de::Visitor};
    use std::fmt;

    pub fn serialize<S>(sig: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&hex::encode(sig))
        } else {
            serializer.serialize_bytes(sig)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SigVisitor;
        impl<'de> Visitor<'de> for SigVisitor {
            type Value = [u8; 64];

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a 64-byte signature or 128-char hex string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let bytes = hex::decode(v).map_err(|e| E::custom(e.to_string()))?;
                if bytes.len() != 64 {
                    return Err(E::custom(format!("expected 64 bytes, got {}", bytes.len())));
                }
                let mut out = [0_u8; 64];
                out.copy_from_slice(&bytes);
                Ok(out)
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                if v.len() != 64 {
                    return Err(E::custom(format!("expected 64 bytes, got {}", v.len())));
                }
                let mut out = [0_u8; 64];
                out.copy_from_slice(v);
                Ok(out)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut out = [0_u8; 64];
                for slot in &mut out {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| Error::custom("expected 64 elements"))?;
                }
                Ok(out)
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(SigVisitor)
        } else {
            deserializer.deserialize_bytes(SigVisitor)
        }
    }
}

pub mod signatures_vec {
    use serde::{Deserializer, Serializer, de::Error, de::SeqAccess, de::Visitor};
    use std::fmt;

    pub fn serialize<S>(sigs: &[[u8; 64]], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(sigs.len()))?;
        for sig in sigs {
            seq.serialize_element(&hex::encode(sig))?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<[u8; 64]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SigsVisitor;
        impl<'de> Visitor<'de> for SigsVisitor {
            type Value = Vec<[u8; 64]>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence of 64-byte signatures")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut out = Vec::new();
                while let Some(elem) = seq.next_element::<serde_json::Value>()? {
                    match elem {
                        serde_json::Value::String(s) => {
                            let bytes =
                                hex::decode(&s).map_err(|e| Error::custom(e.to_string()))?;
                            if bytes.len() != 64 {
                                return Err(Error::custom(format!(
                                    "expected 64 bytes, got {}",
                                    bytes.len()
                                )));
                            }
                            let mut sig = [0_u8; 64];
                            sig.copy_from_slice(&bytes);
                            out.push(sig);
                        }
                        serde_json::Value::Array(arr) => {
                            if arr.len() != 64 {
                                return Err(Error::custom(format!(
                                    "expected 64 elements, got {}",
                                    arr.len()
                                )));
                            }
                            let mut sig = [0_u8; 64];
                            for (i, v) in arr.iter().enumerate() {
                                sig[i] =
                                    v.as_u64().ok_or_else(|| Error::custom("invalid u8"))? as u8;
                            }
                            out.push(sig);
                        }
                        _ => return Err(Error::custom("unexpected signature value")),
                    }
                }
                Ok(out)
            }
        }

        deserializer.deserialize_seq(SigsVisitor)
    }
}
