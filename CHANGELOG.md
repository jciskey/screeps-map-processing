# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [v0.1.0]

Initial release.

### Added

- `CompressedRoomTerrain`, a simple bit-packed representation of room terrain.
- `RoomEdgeTerrain`, a bit-packed representation of room edge terrain data.
- `BinarySearchRLE`, a generic run-length encoding struct that allows for `O(lg(n))` search via binary search.
- `BinarySearchPackedRoomTerrainRLE`, a terrain-specific run-length encoding binary search tree.
- `PackedRLERoomTerrain`, a user-friendly interface for storing and working with compressed room terrain data. Uses run-length encoding internally.
- `WildcardRLERoomTerrain`, a user-friendly interface for storing and working with compressed room terrain data. Uses run-length encoding with wildcards internally.
- `RoomExit`, a compact representation of an individual exit along a room edge.
- `RoomExitsData`, a compact representation of all the exits in a room.
- `compressed_terrain_db`, a module for using SQLite to store and retrieve compressed terrain data.
