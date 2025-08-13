
use crate::run_length_encoding::generic_rle::BinarySearchRLE;

use screeps::{Terrain, LocalRoomTerrain, RoomXY, ROOM_AREA};
use screeps::local::{terrain_index_to_xy, xy_to_terrain_index};

use crate::compressed_terrain::compressed_terrain::CompressedRoomTerrain;

/// RLE-encoded room terrain data, using the [generic_rle](crate::run_length_encoding::generic_rle)
/// submodule.
pub struct RLERoomTerrain {
    data: BinarySearchRLE<Terrain, u16>,
}

impl RLERoomTerrain {
    /// Converts uncompressed terrain data into a compressed RLE-encoded format.
    pub fn new_from_uncompressed_terrain(terrain: &LocalRoomTerrain) -> Self {
        let mut data = BinarySearchRLE::new();

        for idx in 0..ROOM_AREA {
            let xy = terrain_index_to_xy(idx);
            let tile = terrain.get_xy(xy);
            data.append_token(tile, idx as u16);
        }

        Self { data }
    }

    /// Converts bit-packed compressed terrain data into a compressed RLE-encoded format.
    pub fn new_from_compressed_terrain(terrain: &CompressedRoomTerrain) -> Self {
        let mut data = BinarySearchRLE::new();

        for idx in 0..ROOM_AREA {
            let xy = terrain_index_to_xy(idx);
            let tile = terrain.get_xy(xy);
            data.append_token(tile, idx as u16);
        }

        Self { data }
    }

    /// Gets the terrain for a given tile.
    pub fn get_xy(&self, xy: RoomXY) -> Terrain {
        let idx = xy_to_terrain_index(xy);
        // Safety: We'll always be populated with data, so there will always be a result
        self.data.find_token_at_index(idx as u16).unwrap()
    }

    /// Returns the number of distinct runs contained.
    pub fn num_runs(&self) -> usize {
        self.data.num_runs()
    }

    /// The amount of memory it takes to store this data.
    pub fn memory_size(&self) -> usize {
        self.data.memory_size()
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use screeps::constants::{ROOM_AREA, ROOM_SIZE};
	use screeps::LocalRoomTerrain;
    use itertools::Itertools;

    #[test]
    pub fn rle_terrain_addresses_data_in_row_major_order() {
        // Initialize terrain to be all plains
        let mut raw_terrain_data = [0; ROOM_AREA];

        // Adjust (1, 0) to be a swamp; in row-major order this is the second element
        // (index 1) in the array; in column-major order this is the 51st
        // element (index 50) in the array.
        raw_terrain_data[1] = 2; // Terrain::Swamp has the numeric representation 2

        // Construct the local terrain object
        let compressed_terrain = CompressedRoomTerrain::new_from_uncompressed_bits(&raw_terrain_data);

        let terrain = RLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);

        // Pull the terrain for location (1, 0); if it comes out as a Swamp, then we
        // know the get_xy function pulls data in row-major order; if it comes
        // out as a Plain, then we know that it pulls in column-major order.
        let xy = unsafe { RoomXY::unchecked_new(1, 0) };
        let tile_type = terrain.get_xy(xy);
        assert_eq!(Terrain::Swamp, tile_type, "Terrain mismatch at {xy}");
    }

    #[test]
    pub fn rle_terrain_get_xy_matches_uncompressed_terrain() {
        // Initialize terrain to be all plains
        let mut raw_terrain_data = Box::new([0; ROOM_AREA]);

        // Adjust terrain to be heterogeneous
        for i in 0..ROOM_AREA {
            // Safety: mod 3 will always be a valid u8
            let tile_type: u8 = (i % 3) as u8; // Range: 0, 1, 2 -> Plains, Wall, Swamp
            raw_terrain_data[i] = tile_type;
        }

        // Construct the local terrain object
        let terrain = LocalRoomTerrain::new_from_bits(raw_terrain_data);

        // Build the new compressed terrain from the referenced bits
        let new_terrain = RLERoomTerrain::new_from_uncompressed_terrain(&terrain);

        // Iterate over all room positions and verify that they match in both terrain
        // objects
        for x in 0..ROOM_SIZE {
            for y in 0..ROOM_SIZE {
                // Safety: x and y are both explicitly restricted to room size
                let xy = unsafe { RoomXY::unchecked_new(x, y) };
                assert_eq!(terrain.get_xy(xy), new_terrain.get_xy(xy), "Terrain mismatch at {xy}");
            }
        }
    }
}
