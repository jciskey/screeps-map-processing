use std::mem::size_of;
use screeps::{Terrain, RoomXY, ROOM_USIZE};

// The naive encoding is to take the tiles from 1 to 48 and encode them using a single bit each.
// The corners of the room are always Walls, so we can ignore those for the actual data storage.
// 48 bits is 6 bytes, meaning we need 24 bytes per edge to encode all the terrain directly.

/// The errors that can be returned when parsing edge terrain data from slices.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RoomEdgeTerrainParseError {
    TopEdgeNotLength50,
    RightEdgeNotLength50,
    BottomEdgeNotLength50,
    LeftEdgeNotLength50,
}

/// Stores room edge terrain data compressed via bit-packing.
#[derive(Debug, Copy, Clone)]
pub struct RoomEdgeTerrain {
    data: [u8; 24],
}

impl RoomEdgeTerrain {
    /// Creates a new RoomEdgeTerrain from raw compressed data bytes.
    ///
    /// This is primarily useful for reconstituting a RoomEdgeTerrain object from saved raw data.
    /// To create a RoomEdgeTerrain object from regular uncompressed Terrain data, see
    /// [new_from_terrain_slices](RoomEdgeTerrain::new_from_terrain_slices).
    pub fn new_from_raw_bytes(data: [u8; 24]) -> Self {
        Self { data }
    }

    /// Creates a new RoomEdgeTerrain from slices of Terrain data corresponding to each edge of a
    /// room.
    ///
    /// Each slice is expected to be 50 elements in length. Passing slices of shorter or longer
    /// length will return Err.
    ///
    /// Since these are room edges, the only valid Terrain variants are Plains and Walls. Swamps
    /// are considered to be Plains and will be encoded as such. This *will* cause data loss when
    /// converting from Terrain to compressed byte data and back.
    pub fn new_from_terrain_slices(top: &[Terrain], right: &[Terrain], bottom: &[Terrain], left: &[Terrain]) -> Result<Self, RoomEdgeTerrainParseError> {
        if top.len() != 50 {
            return Err(RoomEdgeTerrainParseError::TopEdgeNotLength50);
        }
        if right.len() != 50 {
            return Err(RoomEdgeTerrainParseError::RightEdgeNotLength50);
        }
        if bottom.len() != 50 {
            return Err(RoomEdgeTerrainParseError::BottomEdgeNotLength50);
        }
        if left.len() != 50 {
            return Err(RoomEdgeTerrainParseError::LeftEdgeNotLength50);
        }

        let top_slice = top.try_into().expect("should always be length 50");
        let right_slice = right.try_into().expect("should always be length 50");
        let bottom_slice = bottom.try_into().expect("should always be length 50");
        let left_slice = left.try_into().expect("should always be length 50");
        
        let mut data = [0u8; 24];
        let (chunks, _) = data.as_chunks_mut::<6>();

        for (mut bytes, slice) in chunks.into_iter().zip([top_slice, right_slice, bottom_slice, left_slice]) {
            RoomEdgeTerrain::copy_edge_terrain_to_byte_slice(slice, &mut bytes);
        }

        Ok(Self { data })
    }

    /// Returns the compressed internal representation of the room edge terrain data.
    pub fn get_raw_bytes(&self) -> [u8; 24] {
        self.data
    }

    /// Internal helper function to compress 8 tiles of Terrain into a single byte.
    ///
    /// Valid variants are Plains and Walls. Swamps will be converted silently to Plains.
    fn get_byte_from_terrain(terrain: &[Terrain; 8]) -> u8 {
        let mut output = 0_u8;

        for (i, t) in terrain.iter().enumerate() {
            if *t == Terrain::Wall {
                output = output | (1 << (7 - i));
            }
        }

        output
    }

    /// Internal helper function to write an entire edge of Terrain data into a chunk of 6 u8s.
    fn copy_edge_terrain_to_byte_slice(terrain: &[Terrain; 50], output: &mut [u8; 6]) {
        let (chunks, _) = terrain[1..=48].as_chunks::<8>(); // The two endpoints are always Walls, and thus are not part of the byte data

        for (i, slice) in chunks.iter().enumerate() {
            output[i] = RoomEdgeTerrain::get_byte_from_terrain(slice);
        }
    }

    /// Internal helper function to write a compressed byte of Terrain data into a chunk of 8
    /// Terrain variants.
    fn copy_terrain_from_byte(byte: u8, output: &mut [Terrain; 8]) {
        for i in 0..=7 {
            let bit_idx = 7 - i;
            output[i] = match (byte >> bit_idx) & 1 {
                0 => Terrain::Plain,
                1 => Terrain::Wall,
                _ => unreachable!(), // We're bitmasking against 0b1, it can only ever be 0 or 1
            };
        }
    }

    /// Internal helper function to get an individual tile's terrain directly from a chunk.
    ///
    /// edge_offset is the 0-indexed position of the tile along the edge, in LTR order.
    /// edge_offset should always be in the inclusive range [1,48]
    fn get_tile_terrain_from_chunk(chunk: &[u8; 6], edge_offset: u8) -> Option<Terrain> {
        if (edge_offset < 49) & (edge_offset > 0) {
            // Internal offset: since our data is shifted to the left by 1, since idx 0 on an edge
            // is always a wall
            let offset = (edge_offset - 1) as usize;

            // Determine the byte the tile is in: offset / 8
            let byte = chunk[offset/8]; // Safety: Offset is always LTE 48, so this will always be LTE 6

            // Determine the bit inside the byte the tile corresponds to: offset % 8
            let bit_idx = offset % 8;
            let bitshift = 7 - bit_idx;
            match (byte >> bitshift) & 1 {
                0 => Some(Terrain::Plain),
                1 => Some(Terrain::Wall),
                _ => unreachable!(),
            }
        } else {
            None
        }
    }

    /// Internal helper function that converts a byte of compressed Terrain Data into the
    /// corresponding chunk of Terrain variants.
    fn get_terrain_from_byte(byte: u8) -> [Terrain; 8] {
        let mut output = [Terrain::Plain; 8];
        RoomEdgeTerrain::copy_terrain_from_byte(byte, &mut output);
        output
    }

    /// Helper function for converting a chunk of compressed Terrain data into an edge of Terrain
    /// variants.
    pub fn get_edge_terrain_from_bytes(bytes: &[u8; 6]) -> [Terrain; 50] {
        let mut edge = [Terrain::Wall; 50]; // We're copying everything from 1 to 48, so this saves us having to explicitly write 0 and 49 as Walls

        let (chunks, _) = edge[1..=48].as_chunks_mut::<8>(); // The two endpoints are always Walls, and thus are not part of the byte data
        let iter = std::iter::zip(bytes, chunks);

        for (byte, slice) in iter {
            RoomEdgeTerrain::copy_terrain_from_byte(*byte, slice);
        }

        edge
    }

    /// Internal helper function that gets the slice of compressed data corresponding to the top
    /// edge of the room.
    fn get_top_edge_bytes_slice(&self) -> &[u8; 6] {
        self.data[0..6].try_into().unwrap()
    }

    /// Internal helper function that gets the slice of compressed data corresponding to the right
    /// edge of the room.
    fn get_right_edge_bytes_slice(&self) -> &[u8; 6] {
        self.data[6..12].try_into().unwrap()
    }

    /// Internal helper function that gets the slice of compressed data corresponding to the bottom
    /// edge of the room.
    fn get_bottom_edge_bytes_slice(&self) -> &[u8; 6] {
        self.data[12..18].try_into().unwrap()
    }

    /// Internal helper function that gets the slice of compressed data corresponding to the left
    /// edge of the room.
    fn get_left_edge_bytes_slice(&self) -> &[u8; 6] {
        self.data[18..24].try_into().unwrap()
    }

    /// Returns the Terrain data corresponding to the top edge of the room.
    pub fn get_top_edge_terrain(&self) -> [Terrain; 50] {
        Self::get_edge_terrain_from_bytes(self.get_top_edge_bytes_slice())
    }

    /// Returns the Terrain data corresponding to the right edge of the room.
    pub fn get_right_edge_terrain(&self) -> [Terrain; 50] {
        Self::get_edge_terrain_from_bytes(self.get_right_edge_bytes_slice())
    }

    /// Returns the Terrain data corresponding to the bottom edge of the room.
    pub fn get_bottom_edge_terrain(&self) -> [Terrain; 50] {
        Self::get_edge_terrain_from_bytes(self.get_bottom_edge_bytes_slice())
    }

    /// Returns the Terrain data corresponding to the left edge of the room.
    pub fn get_left_edge_terrain(&self) -> [Terrain; 50] {
        Self::get_edge_terrain_from_bytes(self.get_left_edge_bytes_slice())
    }

    /// Returns the Terrain for the specified tile.
    ///
    /// Returns None if the specified tile is not an edge tile.
    pub fn get_xy(&self, xy: RoomXY) -> Option<Terrain> {
        match (xy.x.u8(), xy.y.u8()) {
            (0, 0) | (0, 49) | (49, 0) | (49, 49) => Some(Terrain::Wall), // Room corners are always walls
            (x, y) if x > 0 && x < 49 && y > 0 && y < 49 => None, // Not an edge
            (x, y) if x > 49 || y > 49 => None, // Not a valid room xy
            (x, y) => {
                let (byte_slice, edge_offset) = match (x, y) {
                    (1..=48, 0) => {
                        // Top edge
                        (self.get_top_edge_bytes_slice(), x)
                    },
                    (1..=48, 49) => {
                        // Bottom edge
                        (self.get_bottom_edge_bytes_slice(), x)
                    },
                    (0, 1..=48) => {
                        // Left edge
                        (self.get_left_edge_bytes_slice(), y)
                    },
                    (49, 1..=48) => {
                        // Right edge
                        (self.get_right_edge_bytes_slice(), y)
                    },
                    _ => unreachable!(), // We can't get here because of prior checks, but the compiler doesn't know that
                };
                Self::get_tile_terrain_from_chunk(byte_slice, edge_offset)
            }
        }
    }

    /// The amount of memory that it takes to hold this data.
    pub fn memory_size(&self) -> usize {
        size_of::<[u8; 24]>()
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use itertools::Itertools;
    use screeps::{Terrain, RoomXY, ROOM_USIZE, ROOM_SIZE, ROOM_AREA, LocalRoomTerrain};

	fn bits_to_byte(bits: &[bool; 8]) -> u8 {
		let mut byte: u8 = 0;
		for (i, &bit) in bits.iter().enumerate() {
			if bit {
				// Set the bit at the correct position (MSB first)
				byte |= 1 << (7 - i); 
			}
		}
		byte
	}

    #[test]
    pub fn room_edge_terrain_copy_terrain_from_byte_copies_data_correctly() {
        // Combinatorically build an array of 8 tiles, all 256 permutations
        let arr: [bool; 2] = [false, true]; // Plains = 0/false, Walls = 1/true
        let edge_segments_iter = itertools::repeat_n(arr, 8).multi_cartesian_product();

        for segment in edge_segments_iter {
            // Convert the Vec permutation into an array
            let arr: [bool; 8] = segment.try_into().unwrap(); // Safety: This should always be an 8-element Vec

            // Convert each array into a byte
            let original_byte = bits_to_byte(&arr);
            let original_terrain = arr.map(|tile| if tile { Terrain::Wall } else { Terrain::Plain });

            // Run the byte through the copy terrain function
            let mut output: [Terrain; 8] = [Terrain::Plain; 8];
            RoomEdgeTerrain::copy_terrain_from_byte(original_byte, &mut output);

            // Verify that the output matches the original byte
            assert_eq!(output, original_terrain);
        }
    }

    #[test]
    pub fn room_edge_terrain_get_byte_from_terrain_calculates_correctly() {
        for original_byte in 0..=u8::MAX {

            let original_terrain = RoomEdgeTerrain::get_terrain_from_byte(original_byte);
            let mut terrain_vec = Vec::new();
            for i in 0..8 {
                let terrain = match (original_byte >> (7 - i)) & 1 {
                    0 => Terrain::Plain,
                    1 => Terrain::Wall,
                    _ => unreachable!(),
                };
                terrain_vec.push(terrain);
            }

            assert_eq!(original_terrain, &terrain_vec[0..8]);

            let output = RoomEdgeTerrain::get_byte_from_terrain(&original_terrain);

            // Verify that the output matches the original byte
            assert_eq!(output, original_byte);
        }
    }

    #[test]
    pub fn room_edge_terrain_copy_edge_terrain_to_byte_slice_copies_data_correctly() {
        let byte_0 = u8::MAX/2;
        let byte_1 = 0_u8;
        let byte_2 = 1_u8;
        let byte_3 = 2_u8;
        let byte_4 = 3_u8;
        let byte_5 = 4_u8;
        let data = [byte_0, byte_1, byte_2, byte_3, byte_4, byte_5];

        let terrain = RoomEdgeTerrain::get_edge_terrain_from_bytes(&data);

        let mut output = [0u8; 6];
        RoomEdgeTerrain::copy_edge_terrain_to_byte_slice(&terrain, &mut output);

        assert_eq!(output, data);
    }

    #[test]
    pub fn room_edge_terrain_edge_slices_match() {
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

        // Pull the terrain for each edge
        let top_vec: Vec<Terrain> = (0..=49).map(|x| {
            let y = 0;
            let xy = unsafe { RoomXY::unchecked_new(x, y) };
            if x == 0 || x == 49 {
                Terrain::Wall
            } else {
                let tile = terrain.get_xy(xy);
                if tile == Terrain::Swamp {
                    Terrain::Plain
                } else {
                    tile
                }
            }
        }).collect();

        let right_vec: Vec<Terrain> = (0..=49).map(|y| {
            let x = 49;
            let xy = unsafe { RoomXY::unchecked_new(x, y) };
            if y == 0 || y == 49 {
                Terrain::Wall
            } else {
                let tile = terrain.get_xy(xy);
                if tile == Terrain::Swamp {
                    Terrain::Plain
                } else {
                    tile
                }
            }
        }).collect();

        let bottom_vec: Vec<Terrain> = (0..=49).map(|x| {
            let y = 49;
            let xy = unsafe { RoomXY::unchecked_new(x, y) };
            if x == 0 || x == 49 {
                Terrain::Wall
            } else {
                let tile = terrain.get_xy(xy);
                if tile == Terrain::Swamp {
                    Terrain::Plain
                } else {
                    tile
                }
            }
        }).collect();

        let left_vec: Vec<Terrain> = (0..=49).map(|y| {
            let x = 0;
            let xy = unsafe { RoomXY::unchecked_new(x, y) };
            if y == 0 || y == 49 {
                Terrain::Wall
            } else {
                let tile = terrain.get_xy(xy);
                if tile == Terrain::Swamp {
                    Terrain::Plain
                } else {
                    tile
                }
            }
        }).collect();

        // Construct the room edge terrain object
        let room_edge_terrain_res = RoomEdgeTerrain::new_from_terrain_slices(&top_vec, &right_vec, &bottom_vec, &left_vec);

        assert!(room_edge_terrain_res.is_ok());

        let room_edge_terrain = room_edge_terrain_res.unwrap();

        // Check that each edge slice matches with the one that was provided
        let top_edge_slice = room_edge_terrain.get_top_edge_terrain();
        let right_edge_slice = room_edge_terrain.get_right_edge_terrain();
        let bottom_edge_slice = room_edge_terrain.get_bottom_edge_terrain();
        let left_edge_slice = room_edge_terrain.get_left_edge_terrain();

        assert_eq!(top_vec, top_edge_slice, "top edge mismatch");
        assert_eq!(right_vec, right_edge_slice, "right edge mismatch");
        assert_eq!(bottom_vec, bottom_edge_slice, "bottom edge mismatch");
        assert_eq!(left_vec, left_edge_slice, "left edge mismatch");
    }

    #[test]
    pub fn room_edge_terrain_get_xy_returns_expected_values() {
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

        // Pull the terrain for each edge
        let top_vec: Vec<Terrain> = (0..=49).map(|x| {
            let y = 0;
            let xy = unsafe { RoomXY::unchecked_new(x, y) };
            terrain.get_xy(xy)
        }).collect();

        let right_vec: Vec<Terrain> = (0..=49).map(|y| {
            let x = 49;
            let xy = unsafe { RoomXY::unchecked_new(x, y) };
            terrain.get_xy(xy)
        }).collect();

        let bottom_vec: Vec<Terrain> = (0..=49).map(|x| {
            let y = 49;
            let xy = unsafe { RoomXY::unchecked_new(x, y) };
            terrain.get_xy(xy)
        }).collect();

        let left_vec: Vec<Terrain> = (0..=49).map(|y| {
            let x = 0;
            let xy = unsafe { RoomXY::unchecked_new(x, y) };
            terrain.get_xy(xy)
        }).collect();

        // Construct the room edge terrain object
        let room_edge_terrain_res = RoomEdgeTerrain::new_from_terrain_slices(&top_vec, &right_vec, &bottom_vec, &left_vec);

        assert!(room_edge_terrain_res.is_ok());

        let room_edge_terrain = room_edge_terrain_res.unwrap();

        // Iterate over all edge positions and verify that they match in both terrain
        // objects
        for x in 0..ROOM_SIZE {
            for y in 0..ROOM_SIZE {
                // Safety: x and y are both explicitly restricted to room size
                let xy = unsafe { RoomXY::unchecked_new(x, y) };
                let ret = room_edge_terrain.get_xy(xy);
                if xy.is_room_edge() {
                    let expected_terrain = match (x, y) {
                        (0, 0) | (0, 49) | (49, 0) | (49, 49) => Some(Terrain::Wall), // Corners are always Walls
                        _ => {
                            let tile = terrain.get_xy(xy);
                            if tile == Terrain::Swamp {
                                Some(Terrain::Plain)
                            } else {
                                Some(tile)
                            }
                        },
                    };

                    assert_eq!(expected_terrain, ret, "non-matching terrain; xy: {xy}");
                } else {
                    assert!(ret.is_none(), "non-edge tiles should return none; xy: {xy}");
                }
            }
        }
    }
}
