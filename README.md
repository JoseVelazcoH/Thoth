<p align="center">
  <picture><source media="(prefers-color-scheme: dark)" srcset="https://shieldcn.dev/header/surface.svg?title=Thoth&amp;subtitle=Your+shell+forgets.+Thoth+doesn%27t.&amp;logo=ri%3ABsTerminal&amp;logoColor=000000&amp;mode=dark&amp;theme=orange&amp;font=geist-mono&amp;border=false" /><img alt="header" src="https://shieldcn.dev/header/surface.svg?title=Thoth&amp;subtitle=Your+shell+forgets.+Thoth+doesn%27t.&amp;logo=ri%3ABsTerminal&amp;logoColor=000000&amp;mode=light&amp;theme=orange&amp;font=geist-mono&amp;border=false" /></picture>
</p>

## What is Thoth?
Thoth is an intelligent shell history. Instead of a flat list of commands, it records each command together with the context it ran in: the working directory, the inferred project, how long it took, its exit code, and the tags of the active work session.

Commands are automatically grouped into work sessions and can be searched by project, tag, date, result, and free text.

The name comes from Thoth, the Egyptian god of writing and memory. The binary is tth.


## Status

Early development. The capture engine is written in Rust and ships as a single static binary (SQLite with full-text search is bundled in). Shell hooks for automatic capture (bash and zsh) and the query commands are in progress.

A Python prototype that validated the original design lives under `prototype/python/`.
