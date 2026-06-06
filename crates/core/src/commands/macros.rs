//! Shared command-generation macros.
//!
//! These collapse the boilerplate that several commands repeat verbatim. The bar for
//! living here is exactness: a macro generates a command only when the hand-written
//! body would be byte-identical to the expansion. A command with extra validation,
//! side effects, or an asymmetric undo stays hand-written — see `set_handle.rs` or
//! `change_relationship_kind.rs`. A near-fit forced through a macro hides the very
//! difference that matters, so don't.

/// Generates a single-field "swap" command: one that replaces one field of a target
/// looked up by id, stashes the old value, and restores it on undo.
///
/// The generated command holds the value in one `Option<$ty>`: it carries the pending
/// value before apply, then the prior value after, so apply and undo are the same swap
/// run twice (`undo` calls `apply`). This is the simplest faithful reverse for a field
/// edit where the UI submits a finished value, and it matches the `detail_command!`
/// shape in `set_details.rs`.
///
/// Use it only for the exact skeleton: `new(id, value)` stores the value, `apply` swaps
/// it into `doc.codex_mut().$accessor(self.id).$target_field`, and the size estimate is
/// `size_of::<Self>()` plus whatever the held value contributes. Anything with extra
/// validation, a no-op fast path, or a three-state prior belongs in its own file.
///
/// Parameters:
/// - `$cmd` — the command type to generate.
/// - `ctor` — how `new` takes its value, picked to match the original signature exactly:
///   `into` for `impl Into<$value_ty>` (so a `&str` arg still works), or `value` for a
///   by-value `$value_ty` arg (so `.collect()` at the call site keeps inferring its
///   target). A uniform `impl Into` arm cannot serve both: it breaks inference for the
///   by-value callers, so the constructor shape is a deliberate per-command choice.
/// - `$id_ty` — the id type the command targets.
/// - `$value_ty` — the field's value type, held in an `Option` for the swap.
/// - `$accessor` — the `Codex` method returning `Option<&mut Target>` for the id.
/// - `$err` — the [`CommandError`](crate::command::CommandError) variant for a missing
///   target; it takes the id.
/// - `$target_field` — the field on the target struct to swap.
/// - `$label` — the stable history label key.
/// - `$held` — a `Fn(&$value_ty) -> usize` giving the held value's heap bytes, added to
///   `size_of::<Self>()` for the size estimate.
macro_rules! swap_field_command {
    (
        $(#[$meta:meta])*
        $cmd:ident,
        ctor: $ctor:ident,
        id: $id_ty:ty,
        value: $value_ty:ty,
        accessor: $accessor:ident,
        not_found: $err:expr,
        field: $target_field:ident,
        label: $label:literal,
        held_size: $held:expr $(,)?
    ) => {
        $(#[$meta])*
        pub struct $cmd {
            id: $id_ty,
            value: Option<$value_ty>,
        }

        impl $cmd {
            swap_field_command!(@ctor $ctor, $id_ty, $value_ty);
        }

        impl $crate::command::Command for $cmd {
            fn apply(&mut self, doc: &mut $crate::document::Document) -> Result<(), $crate::command::CommandError> {
                let value = self.value.take().ok_or($crate::command::CommandError::InvalidState)?;
                let target = doc.codex_mut().$accessor(self.id).ok_or($err(self.id))?;
                self.value = Some(std::mem::replace(&mut target.$target_field, value));
                doc.bump_revision();
                Ok(())
            }

            fn undo(&mut self, doc: &mut $crate::document::Document) -> Result<(), $crate::command::CommandError> {
                // apply and undo are symmetric: each swaps the held value with the live one.
                self.apply(doc)
            }

            fn label_key(&self) -> &'static str {
                $label
            }

            fn estimated_size_bytes(&self) -> usize {
                std::mem::size_of::<Self>() + self.value.as_ref().map_or(0, $held)
            }
        }
    };

    // `into`: accept any `impl Into<$value_ty>`, matching the rename commands' original
    // `&str`-accepting signature.
    (@ctor into, $id_ty:ty, $value_ty:ty) => {
        /// A command that will set the target `id`'s field to `value`. Undo restores
        /// the prior value.
        pub fn new(id: $id_ty, value: impl Into<$value_ty>) -> Self {
            Self { id, value: Some(value.into()) }
        }
    };

    // `value`: take the value by its concrete type, matching the fragment commands'
    // original signature so a `.collect()` argument still infers its target.
    (@ctor value, $id_ty:ty, $value_ty:ty) => {
        /// A command that will set the target `id`'s field to `value`. Undo restores
        /// the prior value.
        pub fn new(id: $id_ty, value: $value_ty) -> Self {
            Self { id, value: Some(value) }
        }
    };
}

// Crate-internal re-export so consumers can `use crate::commands::macros::swap_field_command;`
// instead of relying on `#[macro_use]` ordering, which is fragile across module moves.
pub(crate) use swap_field_command;
