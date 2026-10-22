# Teramic Runtime Patch

Vendored unchanged from `tilt-ui-runtime 0.1.0` on crates.io before the local
changes below. Upstream source commit:
`20995eb12cb7591996a0b60af4d7fe4c8193f507` (`crates/tilt-ui-runtime`).
The upstream MIT/Apache-2.0 license texts are included. No dependency version,
public binding API, template syntax, widget feature, or frame pacing changed.

## Binding Snapshot Ownership

- Simple dotted paths borrow their root and traverse borrowed JSON, cloning
  only the returned leaf. Previously every `inventoryView.title` expression
  cloned the whole inventory, including its item catalog.
- Compound expressions also traverse borrowed roots and members using an
  internal `Cow<Value>` path. Grammar, operator evaluation and short circuiting
  are unchanged. Methods still receive the resolved receiver and owned arguments.
  Local scopes, explicit locals, template imports, store aliases, array indices,
  and string/array lengths keep their precedence. Computed results are owned;
  returning a whole container intentionally still clones that returned value.
- Shared snapshots update only when the corresponding resource changes or
  appears/disappears. An unrelated HUD update no longer serializes every
  inventory, recipe, menu, and map resource again.
- A snapshot's aliases share one allocation. Rebuilding the alias index retains
  registry-order collision precedence and does not copy unchanged JSON trees.
- An equal serialized value retains its previous allocation and does not
  increment the global binding revision.
- Reactive flow nodes cache their hierarchy-depth sort keys for each update.
  Parent chains are traversed once per pending node, not again for every sort
  comparison; the existing stable parent-before-child ordering is preserved.

Regression tests cover large unchanged stores during repeated small updates,
same-JSON changes, resource removal/reinsertion, alias collisions, JSON null,
borrowed root identity, local shadowing, Unicode lengths, owned/borrowed compound
equivalence, indexed traversal, method receivers, and short circuit call counts.

The four original showcase test fixtures omitted from the crates.io package
are included under `tests/fixtures/basic` from the same upstream commit above.
Only their `include_str!` paths were adjusted; the fixtures and original tests
are unchanged. Run the dependency's tests from the Teramic workspace, with its
existing lockfile and resolved features:

```sh
cargo test --locked -p tilt-ui-runtime --lib
```
