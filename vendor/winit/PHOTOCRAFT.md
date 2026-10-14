# PhotoCraft Android input patch

Vendored crates.io winit 0.30.13 (Apache-2.0, see LICENSE). Original `.crate`
SHA-256: `a6755fa58a9f8350bd1e472d4c3fcc25f824ec358933bba33306d0b63df5978d`.
Unchanged except
`src/platform_impl/android/mod.rs`: Android `Touch.force` is `Some` only for
Stylus/Eraser/Mouse, and `None` for other tools. Android finger pressure is not evidence
of a pen. This preserves the distinction through egui-winit without a public API
change or unsafe application code. Other platforms are unchanged.

PhotoCraft uses this contract only in its Android input adapter. Revisit when
upgrading winit to an API that carries pointer/tool type explicitly.
