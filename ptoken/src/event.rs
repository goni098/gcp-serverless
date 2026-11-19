//! Each event is emitted as one base64 `Program data:` field. The decoded
//! bytes start with a one-byte Ptoken tag (0 for initialization, 1 for a
//! deposit, 2 for a withdrawal), followed by the fields in declaration order.
//! Addresses are raw 32-byte keys and transfer amounts are little-endian u64s.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct VaultInitialized {
    pub caller: [u8; 32],
    pub mint: [u8; 32],
}

impl VaultInitialized {
    pub(crate) const TAG: u8 = 0;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct VaultDeposited {
    pub caller: [u8; 32],
    pub mint: [u8; 32],
    pub amount: u64,
}

impl VaultDeposited {
    pub(crate) const TAG: u8 = 1;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct VaultWithdrawn {
    pub caller: [u8; 32],
    pub mint: [u8; 32],
    pub destination: [u8; 32],
    pub amount: u64,
}

impl VaultWithdrawn {
    pub(crate) const TAG: u8 = 2;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PtokenEvent {
    VaultInitialized(VaultInitialized),
    VaultDeposited(VaultDeposited),
    VaultWithdrawn(VaultWithdrawn),
}

impl PtokenEvent {
    pub const MAX_ENCODED_LEN: usize = size_of::<Self>();

    /// Write the event to a caller-provided buffer and return the bytes used.
    pub fn encode<'buffer>(
        &self,
        buffer: &'buffer mut [u8; Self::MAX_ENCODED_LEN],
    ) -> &'buffer [u8] {
        let (tag, payload) = match self {
            Self::VaultInitialized(event) => (VaultInitialized::TAG, bytemuck::bytes_of(event)),
            Self::VaultDeposited(event) => (VaultDeposited::TAG, bytemuck::bytes_of(event)),
            Self::VaultWithdrawn(event) => (VaultWithdrawn::TAG, bytemuck::bytes_of(event)),
        };
        encode_payload(tag, payload, buffer)
    }

    /// Decode raw event bytes, returning `None` for unknown tags or malformed payloads.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let (&tag, payload) = bytes.split_first()?;
        match tag {
            VaultInitialized::TAG => bytemuck::try_pod_read_unaligned(payload)
                .ok()
                .map(Self::VaultInitialized),
            VaultDeposited::TAG => bytemuck::try_pod_read_unaligned(payload)
                .ok()
                .map(Self::VaultDeposited),
            VaultWithdrawn::TAG => bytemuck::try_pod_read_unaligned(payload)
                .ok()
                .map(Self::VaultWithdrawn),
            _ => None,
        }
    }
}

pub(crate) fn encode_payload<'buffer>(
    tag: u8,
    payload: &[u8],
    buffer: &'buffer mut [u8; PtokenEvent::MAX_ENCODED_LEN],
) -> &'buffer [u8] {
    let end = 1 + payload.len();
    buffer[0] = tag;
    buffer[1..end].copy_from_slice(payload);
    &buffer[..end]
}

/// Encode and log an event struct directly.
macro_rules! emit {
    ($event:ident { $($fields:tt)* } $(,)?) => {{
        let event = $crate::event::$event { $($fields)* };
        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        {
            let mut buffer = [0; $crate::event::PtokenEvent::MAX_ENCODED_LEN];
            let fields = [$crate::event::encode_payload(
                $crate::event::$event::TAG,
                ::bytemuck::bytes_of(&event),
                &mut buffer,
            )];
            // SAFETY: The slice descriptor and its event bytes stay live through the syscall.
            unsafe { ::pinocchio::syscalls::sol_log_data(fields.as_ptr().cast(), fields.len() as u64) };
        }
        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        let _ = event;
    }};
}

pub(crate) use emit;

#[cfg(test)]
pub(crate) fn assert_event(mollusk: &mollusk_svm::Mollusk, expected: PtokenEvent) {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    let logs = mollusk.logger.as_ref().unwrap().borrow();
    let payload = logs
        .get_recorded_content()
        .iter()
        .rev()
        .find_map(|line| line.strip_prefix("Program data: "))
        .expect("missing event log");
    let bytes = STANDARD.decode(payload).unwrap();
    assert_eq!(PtokenEvent::decode(&bytes), Some(expected));
}

#[cfg(test)]
#[test]
fn event_decoder_rejects_invalid_tags_and_lengths() {
    let mut buffer = [0; PtokenEvent::MAX_ENCODED_LEN];
    let event = PtokenEvent::VaultInitialized(VaultInitialized::zeroed());
    let encoded = event.encode(&mut buffer);
    assert_eq!(encoded.len(), 1 + size_of::<VaultInitialized>());
    assert_eq!(encoded[0], 0);
    assert_eq!(PtokenEvent::decode(encoded), Some(event));
    assert_eq!(PtokenEvent::decode(&encoded[..encoded.len() - 1]), None);
    assert_eq!(PtokenEvent::decode(&buffer), None);
    buffer[0] = u8::MAX;
    assert_eq!(PtokenEvent::decode(&buffer), None);
}
