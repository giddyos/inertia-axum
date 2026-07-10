# Architecture

```text
HeaderMap
   ↓
RequestContext
   ↓
partial-reload inclusion decision
   ↓
resolved props + consumed metadata
   ↓
PageDraft
   ↓
route-local shared props
   ↓
global shared props
   ↓
Page
   ├── JSON response
   └── script-safe HTML data-page
```

`RequestContext` parses protocol headers once. Both ordinary serializable props
and `InertiaProps` use the same inclusion rules for partial, deferred, once,
always, and optional props. The final page is rendered either as JSON for an
Inertia visit or as script-safe JSON for an HTML shell.
