# Root templates

An Inertia root renderer creates the complete HTML document for an initial browser request. Later visits carrying `X-Inertia: true` return JSON page objects, so they do not invoke the root renderer or recreate the document.

## Built-in root

The built-in root is the default and requires no template file or template-engine dependency:

```rust
let inertia = InertiaApp::vite("frontend").build()?;
```

It writes the required metadata, asset tags, Inertia head output, and CSR or SSR mount directly into one exact-capacity string. Use it for the fastest setup and when application-level document variables are unnecessary.

## Marker-based templates

Marker templates customize static document HTML without a template engine:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <!-- inertia:assets -->
    <!-- inertia:head -->
  </head>
  <body><!-- inertia:mount --></body>
</html>
```

Configure a file with `.root_template("templates/app.html")` or embedded text with `.root_template_source(include_str!("../templates/app.html"))`. Each marker is required exactly once. At startup, the file is read and validated and its static segments are compiled into offsets. Requests only copy those segments and generated fragments into an exact-capacity buffer; they never read or parse the template. Restart after editing a file template.

## Askama templates

Enable `features = ["askama"]`, derive a concrete template through the supported re-export, implement `AskamaRoot`, and register it with `.askama_root(...)`:

```rust
use inertia_axum::{
    AskamaRoot, AskamaRootContext,
    askama::{self, Template},
};

#[derive(Template)]
#[template(path = "app.html", askama = askama)]
struct AppTemplate<'a> {
    inertia: AskamaRootContext<'a>,
    app_name: &'a str,
}
```

The template is compiled into Rust with the application. `AskamaRoot`'s generic associated template type allows each render to borrow both stable application configuration and request-local Inertia fragments. See [`examples/axum-askama`](../examples/axum-askama) for the complete factory, builder setup, and runnable frontend.

The root selection methods `.root(...)`, `.root_template(...)`, `.root_template_source(...)`, and `.askama_root(...)` use last-call-wins semantics.

## Trusted Inertia markup

These three fields are pre-rendered, trusted markup produced by configured framework components:

```html
{{ inertia.assets|safe }}
{{ inertia.head|safe }}
{{ inertia.mount|safe }}
```

Assets come from the configured `AssetProvider`, SSR head and body markup come from the configured SSR backend, and the CSR mount contains script-safe initial-page serialization. `AskamaRootContext` borrows these values; it does not clone them. Page props are not exposed directly to the root template.

## HTML escaping

Askama's HTML escaping remains enabled. Use `safe` only for the three trusted Inertia fields above. Do not mark application strings, user content, metadata, or arbitrary values as safe, and do not disable escaping for the template. Values such as app names, descriptions, locales, and social-image URLs should use normal expressions such as `{{ description }}`.

## SSR behavior

In CSR mode, `inertia.head` is empty and `inertia.mount` contains the initial page JSON plus an empty application element. In SSR mode, the same final root-rendering path receives the backend's head fragments and rendered body mount.

Use `inertia.has_ssr_head` to avoid a duplicate fallback title:

```html
{% if !inertia.has_ssr_head %}
  <title>{{ app_name }}</title>
{% endif %}
{{ inertia.head|safe }}
```

## Performance characteristics

The built-in and marker paths do not compile Askama. The marker path performs all parsing at startup. With the feature enabled, Askama compiles templates into concrete Rust types and request handling calls `render_into` with a buffer preallocated from Askama's `SIZE_HINT` plus the exact generated-fragment lengths. There is no request-time template file access, parsing, dynamic template dispatch, or intermediate rendered string in the adapter.

## Custom RootView implementations

Implement `RootView` when an application needs another engine or complete rendering control. `RootContext` provides borrowed assets, head markup, mount markup, and optional document metadata. Custom roots continue through the same initial-response pipeline; Inertia JSON responses bypass them. Existing custom implementations are unaffected by the optional Askama integration.
