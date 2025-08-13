//! Specialized room terrain that compresses data using Run Length Encoding and wildcards.

use screeps::{Terrain, LocalRoomTerrain, RoomXY, ROOM_AREA};
use screeps::local::{terrain_index_to_xy, xy_to_terrain_index};

use crate::compressed_terrain::compressed_terrain::CompressedRoomTerrain;
use crate::compressed_terrain::compressed_room_edge_terrain::RoomEdgeTerrain;
use super::BinarySearchPackedRoomTerrainRLE;


/// User-friendly interface for getting terrain data.
///
/// Uses [BinarySearchPackedRoomTerrainRLE] internally to store data efficiently, while also using
/// [RoomEdgeTerrain] to store edge terrain data compactly, allowing for all edge tiles to be
/// considered wildcards in the RLE terrain data.
pub struct WildcardRLERoomTerrain {
    data: BinarySearchPackedRoomTerrainRLE,
    edge_data: RoomEdgeTerrain,
}

impl WildcardRLERoomTerrain {
    /// Converts uncompressed room terrain data into a RLE-compressed format with wildcards.
    pub fn new_from_uncompressed_terrain(terrain: &LocalRoomTerrain) -> Self {
        let mut data = BinarySearchPackedRoomTerrainRLE::new();
        let mut top_edge_terrain = Vec::new();
        let mut right_edge_terrain = Vec::new();
        let mut bottom_edge_terrain = Vec::new();
        let mut left_edge_terrain = Vec::new();

        for idx in 0..ROOM_AREA {
            let xy = terrain_index_to_xy(idx);
            let tile = terrain.get_xy(xy);
            
            if xy.is_room_edge() {
                match (xy.x.u8(), xy.y.u8()) {
                    (0, 0) => {
                        // Top-left corner
                        top_edge_terrain.push(Terrain::Wall);
                        left_edge_terrain.push(Terrain::Wall);
                    },
                    (0, 49) => {
                        // Bottom-left corner
                        bottom_edge_terrain.push(Terrain::Wall);
                        left_edge_terrain.push(Terrain::Wall);
                    },
                    (49, 0) => {
                        // Top-right corner
                        top_edge_terrain.push(Terrain::Wall);
                        right_edge_terrain.push(Terrain::Wall);
                    },
                    (49, 49) => {
                        // Bottom-right corner
                        bottom_edge_terrain.push(Terrain::Wall);
                        right_edge_terrain.push(Terrain::Wall);
                    },
                    (1..=48, 0) => {
                        // Top edge
                        top_edge_terrain.push(tile);
                    },
                    (1..=48, 49) => {
                        // Bottom edge
                        bottom_edge_terrain.push(tile);
                    },
                    (0, 1..=48) => {
                        // Left edge
                        left_edge_terrain.push(tile);
                    },
                    (49, 1..=48) => {
                        // Right edge
                        right_edge_terrain.push(tile);
                    },
                    _ => {}, // Not an edge tile
                };
            } else {
                // Skipping adding edge tiles to our RLE data effectively treats them as wildcards
                // that match to the previous run.
                data.append_token(tile, idx as u16);
            }
        }

        // Safety: We constructed this from scratch, we know the data going in is valid
        let edge_data = RoomEdgeTerrain::new_from_terrain_slices(&top_edge_terrain, &right_edge_terrain, &bottom_edge_terrain, &left_edge_terrain).unwrap_or(RoomEdgeTerrain::new_from_raw_bytes([0u8; 24]));

        Self { data, edge_data }
    }

    /// Converts bit-packed compressed terrain into a RLE-compressed format.
    pub fn new_from_compressed_terrain(terrain: &CompressedRoomTerrain) -> Self {
        let mut data = BinarySearchPackedRoomTerrainRLE::new();
        let mut top_edge_terrain = Vec::new();
        let mut right_edge_terrain = Vec::new();
        let mut bottom_edge_terrain = Vec::new();
        let mut left_edge_terrain = Vec::new();

        for idx in 0..ROOM_AREA {
            let xy = terrain_index_to_xy(idx);
            let tile = terrain.get_xy(xy);
            
            if xy.is_room_edge() {
                match (xy.x.u8(), xy.y.u8()) {
                    (0, 0) => {
                        // Top-left corner
                        top_edge_terrain.push(Terrain::Wall);
                        left_edge_terrain.push(Terrain::Wall);
                    },
                    (0, 49) => {
                        // Bottom-left corner
                        bottom_edge_terrain.push(Terrain::Wall);
                        left_edge_terrain.push(Terrain::Wall);
                    },
                    (49, 0) => {
                        // Top-right corner
                        top_edge_terrain.push(Terrain::Wall);
                        right_edge_terrain.push(Terrain::Wall);
                    },
                    (49, 49) => {
                        // Bottom-right corner
                        bottom_edge_terrain.push(Terrain::Wall);
                        right_edge_terrain.push(Terrain::Wall);
                    },
                    (1..=48, 0) => {
                        // Left edge
                        left_edge_terrain.push(tile);
                    },
                    (1..=48, 49) => {
                        // Right edge
                        right_edge_terrain.push(tile);
                    },
                    (0, 1..=48) => {
                        // Left edge
                        left_edge_terrain.push(tile);
                    },
                    (49, 1..=48) => {
                        // Right edge
                        right_edge_terrain.push(tile);
                    },
                    _ => {}, // Not an edge tile
                };
            } else {
                // Skipping adding edge tiles to our RLE data effectively treats them as wildcards
                // that match to the previous run.
                data.append_token(tile, idx as u16);
            }
        }
        
        // Safety: We constructed this from scratch, we know the data going in is valid
        let edge_data = RoomEdgeTerrain::new_from_terrain_slices(&top_edge_terrain, &right_edge_terrain, &bottom_edge_terrain, &left_edge_terrain).unwrap_or(RoomEdgeTerrain::new_from_raw_bytes([0u8; 24]));

        Self { data, edge_data }
    }

    /// Gets the terrain value for the specified tile.
    pub fn get_xy(&self, xy: RoomXY) -> Terrain {
        if xy.is_room_edge() {
            self.edge_data.get_xy(xy).unwrap_or(Terrain::Wall)
        } else {
            let idx = xy_to_terrain_index(xy);
            // Safety: We'll always be populated with data, so there will always be a result
            self.data.find_token_at_index(idx as u16).unwrap()
        }
    }

    /// Returns the number of distinct runs contained.
    pub fn num_runs(&self) -> usize {
        self.data.num_runs()
    }

    /// The amount of memory it takes to store this data.
    pub fn memory_size(&self) -> usize {
        self.data.memory_size() + self.edge_data.memory_size()
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use screeps::constants::{ROOM_AREA, ROOM_SIZE};
	use screeps::{LocalRoomTerrain, xy_to_terrain_index, RoomXY};
    use itertools::Itertools;

    #[test]
    pub fn wildcard_rle_terrain_get_xy_matches_uncompressed_terrain() {
        // Initialize terrain to be all plains
        let mut raw_terrain_data = Box::new([0; ROOM_AREA]);

        // Adjust terrain to be heterogeneous
        for i in 0..ROOM_AREA {
            // Safety: mod 3 will always be a valid u8
            let tile_type: u8 = (i % 3) as u8; // Range: 0, 1, 2 -> Plains, Wall, Swamp
            raw_terrain_data[i] = tile_type;
        }

        // -- Corners are always Walls
        let top_left_corner = unsafe { RoomXY::unchecked_new(0, 0) };
        let top_right_corner = unsafe { RoomXY::unchecked_new(49, 0) };
        let bottom_right_corner = unsafe { RoomXY::unchecked_new(49, 49) };
        let bottom_left_corner = unsafe { RoomXY::unchecked_new(0, 49) };

        let top_left_idx = xy_to_terrain_index(top_left_corner);
        let top_right_idx = xy_to_terrain_index(top_right_corner);
        let bottom_right_idx = xy_to_terrain_index(bottom_right_corner);
        let bottom_left_idx = xy_to_terrain_index(bottom_left_corner);

        raw_terrain_data[top_left_idx] = 1;
        raw_terrain_data[top_right_idx] = 1;
        raw_terrain_data[bottom_right_idx] = 1;
        raw_terrain_data[bottom_left_idx] = 1;

        // Construct the local terrain object
        let terrain = LocalRoomTerrain::new_from_bits(raw_terrain_data);

        // Build the new compressed terrain from the referenced bits
        let new_terrain = WildcardRLERoomTerrain::new_from_uncompressed_terrain(&terrain);

        // Iterate over all room positions and verify that they match in both terrain
        // objects
        for x in 0..ROOM_SIZE {
            for y in 0..ROOM_SIZE {
                // Safety: x and y are both explicitly restricted to room size
                let xy = unsafe { RoomXY::unchecked_new(x, y) };
                let tile = terrain.get_xy(xy);
                let expected_terrain = if xy.is_room_edge() {
                    if tile == Terrain::Swamp {
                        Terrain::Plain // Swamps on edges are actually Plains
                    } else {
                        tile
                    }
                } else {
                    tile
                };

                assert_eq!(expected_terrain, new_terrain.get_xy(xy), "Terrain mismatch at {xy}");
            }
        }
    }
}
