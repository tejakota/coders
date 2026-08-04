---
name: code-analysis
description: Use when asked to review, explain, audit, or understand existing code - Python, JavaScript, TypeScript, HTML, Dart, Rust, or another language. Covers structure mapping, bug/smell hunting, and dependency tracing.
---

Before commenting on code, actually look at it: use `find` to locate relevant
files, `grep` to trace symbol usage and call sites, and `read_file` to read
full context around anything you're about to judge. Never speculate about
code you haven't read.

## General approach

1. Map structure first: entry points, module boundaries, how data flows in
   and out. Use `find` for file layout, `grep` for where a symbol is
   defined vs. where it's used.
2. Read the whole unit (function, component, module) you're analyzing, not
   just the lines someone pasted — surrounding context changes conclusions.
3. Separate findings into: correctness bugs, unhandled edge cases, and
   style/idiom deviations. Don't bury a real bug in a pile of nitpicks.
4. Trace one level of blast radius: if you're flagging a function, `grep`
   for its callers before asserting a fix is safe.
5. Cite `file:line` for every claim so it's checkable.

## Language-specific signals to check

**Python** — mutable default arguments; bare `except:`; missing
`__init__.py` vs. implicit namespace packages; sync I/O inside `async def`;
off-by-one in slicing; type hints that don't match actual returned types;
comparing with `is` on non-singletons.

**JavaScript / TypeScript** — `==` vs `===`; unhandled promise rejections
(missing `await`, floating promises); `any` used to silence real type
errors; stale closures in `useEffect`/event handlers capturing old state;
array mutation methods (`sort`, `splice`) used where immutability was
assumed elsewhere; `for...in` over arrays instead of `for...of`.

**HTML** — missing `alt` text and form labels (a11y); unescaped
user-controlled content injected via `innerHTML` (XSS); duplicate `id`
attributes; scripts blocking render that could be deferred.

**Dart / Flutter** — widgets rebuilt unnecessarily because state lives too
high in the tree; missing `dispose()` for controllers/streams/listeners;
`late` fields accessed before initialization; using `StatefulWidget` where
value equality (`==`)/`copyWith` immutable state would be simpler and
safer; unguarded `!` null-assertions.

**Rust** — unnecessary `.clone()`/`.unwrap()` where a `Result`/reference
would do; lifetimes elided in a way that forces awkward callers; `unsafe`
blocks without a comment justifying the invariant being upheld; blocking
calls inside `async fn`; `Mutex`/`RwLock` held across an `.await` point.

## Output

Lead with the highest-severity finding. For each: what's wrong, exactly
where (`file:line`), what input/state triggers it, and the smallest fix —
not a rewrite unless the whole approach is broken.
