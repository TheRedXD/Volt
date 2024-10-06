# TODO
This is a list of things to do (or things that have been done) for Volt!

- ✔️ - Done
- ❌ - TODO (Not done)
- ❗ - Blocked
- 🔁 - In progress

| Status | Platform |    Category    | Description |
|--------|----------|----------------|-------------|
|   ✔️   | All      | Rendering      | Render Background
|   ✔️   | All      | Rendering      | Render Navbar
|   ✔️   | All      | Rendering      | Render Browser
|   ❌   | All      | Rendering      | Render Playlist
|   ✔️   | All      | Browser        | Fix mouse cursor not staying on horizontal drag when resizing the browser
|   ✔️   | All      | Browser        | Make browser resizable to practically any width within the viewport
|   ❌   | All      | Preview        | FIXME: Temporary rodio playback, might need to use cpal or make rodio proper (browser.rs:13, browser.rs:492)
|   ❌   | All      | Browser        | FIXME: THIS NEEDS TO BE FIXED ASAP, the ordering is wrong (browser.rs:48)
|   ❌   | All      | Window         | Make the window have a proper icon
|   ❌   | All      | Browser        | Optimize the browser (don't read the folders every frame god damnit)
|   ❌   | All      | Browser        | Fix sorting and use [https://docs.rs/indextree](https://docs.rs/indextree)
|   ❌   | Windows  | Browser        | TODO: Enable drag and drop on Windows (browser.rs:223)
|   ❌   | All      | Browser        | TODO: make these two comparisons part of the `rect.contains` check (browser.rs:480)
|   ❌   | All      | Browser        | TODO: Show some devices here! (browser.rs:507)
|   ❌   | All      | CLI            | TODO: could use the `human_panic` crate (info.rs:157)
|   🔁   | All      | All            | Componentize the entire UI
|   ❗   | All      | Navbar         | Make navbar fully line up with the top of the browser (blocked by componentization)
|   ❗   | All      | Playlist       | Draw playlist (blocked by componentization)