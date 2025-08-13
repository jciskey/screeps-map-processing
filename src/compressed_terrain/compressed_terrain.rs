use screeps::local::xy_to_terrain_index;
use screeps::{ROOM_SIZE, ROOM_AREA, RoomXY, Terrain};

/// The size of the internal data array for [CompressedRoomTerrain].
pub const COMPRESSED_ARRAY_SIZE: usize = (ROOM_AREA / 4) as usize; // We pack 4 terrain positions into 1 byte, so our array is 4 times smaller. This should be 625 as the final value.

/// Room terrain that has been compressed via bit-packing.
pub struct CompressedRoomTerrain {
    data: Box<[u8; COMPRESSED_ARRAY_SIZE]>,
}

impl CompressedRoomTerrain {
    /// Gets the terrain at the specified position in this room.
    pub fn get_xy(&self, xy: RoomXY) -> Terrain {
        let byte = self.get_uncompressed_terrain_byte(xy);
        // not using Terrain::from_u8() because `0b11` value, wall+swamp, happens
        // in commonly used server environments (notably the private server default
        // map), and is special-cased in the engine code; we special-case it here
        match byte & 0b11 {
            0b00 => Terrain::Plain,
            0b01 | 0b11 => Terrain::Wall,
            0b10 => Terrain::Swamp,
            // Should be optimized out
            _ => unreachable!("all combinations of 2 bits are covered"),
        }
    }

    /// Gets the internal terrain byte of the specified position.
    fn get_uncompressed_terrain_byte(&self, xy: RoomXY) -> u8 {
        // Determine the linear index of the xy coordinate in an uncompressed array of size 2500
        let uncompressed_index = xy_to_terrain_index(xy);

        // Determine the byte and the internal byte offset corresponding to the uncompressed linear
        // index.
        // 
        // The byte index is the linear index / 4, since terrain data is u2, and we're packing it
        // into a u8.
        //
        // The internal byte offset is linear index % 4, since we're packing 4 of them into each
        // byte, starting at index 0 for input and output.
        let (byte_index, internal_offset) = div_rem(uncompressed_index, 4);

        // Pull the compressed byte
        let raw_byte = self.data[byte_index];

        // Extract the terrain byte from the compressed byte
        let bitshift_amount = match internal_offset {
            0 => 6,
            1 => 4,
            2 => 2,
            3 => 0,
            // This should get optimized away
            _ => unreachable!("all offsets are covered"),
        };

        // After the bitshift, we only want the 2 least significant bits
        let mask = 0b11u8;

        // Shift the relevant bits to the 2 least significant bit positions, then mask off any
        // other more significant bits to leave us with the uncompressed terrain byte
        (raw_byte >> bitshift_amount) & mask
	}

    /// Compresses 4 bytes of raw terrain data into a single byte.
    ///
    /// Note: This will only utilize the 2 least significant bits in any of the 4 bytes; all other
    /// bits will get discarded. This is valid for terrain data, but is _not_ valid for anything
    /// that isn't u2-sized.
	fn compress_4_bytes(bytes: &[u8]) -> u8 {
        let mut working_bytes = [0; 4];
        for i in 0..bytes.len().min(4) {
            working_bytes[i] = bytes[i];
        }

        let mask = 0b11u8; // Mask to truncate all but the 2 least significant bits
        let first_byte_bits = (working_bytes[0] & mask) << 6;
        let second_byte_bits = (working_bytes[1] & mask) << 4;
        let third_byte_bits = (working_bytes[2] & mask) << 2;
        let fourth_byte_bits = working_bytes[3] & mask;

        first_byte_bits | second_byte_bits | third_byte_bits | fourth_byte_bits
	}

    /// Converts a compressed byte of 4 tiles of terrain data into an array of uncompressed terrain
    /// data.
    fn uncompress_byte(byte: u8) -> [u8; 4] {
        let mask = 0b11u8;
        [
            (byte >> 6) & mask,
            (byte >> 4) & mask,
            (byte >> 2) & mask,
            byte & mask,
        ]
    }

    /// Creates a `CompressedRoomTerrain` from uncompressed raw room terrain bits.
	pub fn new_from_uncompressed_bits(bits: &[u8; ROOM_AREA]) -> Self {
        let mut compressed_data: Box<[u8; COMPRESSED_ARRAY_SIZE]> = Box::new([0; COMPRESSED_ARRAY_SIZE]);
        let mut i = 0;
        for chunk in bits.chunks_exact(4) {
            let compressed_byte = Self::compress_4_bytes(chunk);
            compressed_data[i] = compressed_byte;
            i += 1;
        }
        Self { data: compressed_data }
    }

    /// Creates a `CompressedRoomTerrain` from compressed bytes of room terrain data.
    pub fn new_from_compressed_bytes(data: Box<[u8; COMPRESSED_ARRAY_SIZE]>) -> Self {
        Self { data }
    }

    /// Gets a reference to the underlying compressed terrain data.
    pub fn get_compressed_bytes(&self) -> &[u8; COMPRESSED_ARRAY_SIZE] {
        &self.data
    }

    /// Converts the compressed terrain data into uncompressed terrain data.
    pub fn get_uncompressed_bits(&self) -> Box<[u8; ROOM_AREA]> {
		// The general algorithm here is to uncompress each byte in our data array and store it in
        // the output array.

        // Set up the new output array of uncompressed terrain bytes
        let mut uncompressed_bits = Box::new([0; ROOM_AREA]);

        // Using an iterator of chunks lets us bypass manual math drudgery in our loop; mutable
        // slices let us modify the array directly via sane, logical offset constants
        let mut uncompressed_bits_chunks = uncompressed_bits.chunks_exact_mut(4);

        // Uncompress and copy the bytes into the output array
        for compressed_index in 0..self.data.len() {
            let compressed_byte = self.data[compressed_index];
            let uncompressed_bytes = Self::uncompress_byte(compressed_byte);
            
            // Because ROOM_AREA and COMPRESSED_ARRAY_SIZE are linked together via the chunk size
            // of 4, the iterator should always have exactly the right number of elements, and thus
            // we should never get a None in our loop. However, the compiler doesn't quite know
            // that, so we write this the safe way, with a loop break if it somehow hits that edge
            // case against all odds
            if let Some(target_slice) = uncompressed_bits_chunks.next() {
                target_slice[0] = uncompressed_bytes[0];
                target_slice[1] = uncompressed_bytes[1];
                target_slice[2] = uncompressed_bytes[2];
                target_slice[3] = uncompressed_bytes[3];
            } else {
                // We should never get here, since our chunking should always match, but just in
                // case, stop the loop
                break;
            }
        }

        // Toss the caller their uncompressed, inefficient array of terrain data
        uncompressed_bits
    }

    /// The amount of memory it takes to store this data.
    pub fn memory_size(&self) -> usize {
        size_of::<[u8; COMPRESSED_ARRAY_SIZE]>() + size_of::<Box<[u8; COMPRESSED_ARRAY_SIZE]>>()
    }
}

/// Calculates the quotent and remainder. Returned tuple is (quotent, remainder).
pub fn div_rem<T: std::ops::Div<Output=T> + std::ops::Rem<Output=T> + Copy>(x: T, y: T) -> (T, T) {
    let quot = x / y;
    let rem = x % y;
    (quot, rem)
}

#[cfg(test)]
mod test {
    use super::*;
    use screeps::constants::{ROOM_AREA, ROOM_SIZE};
	use screeps::LocalRoomTerrain;
    use itertools::Itertools;

    #[test]
    pub fn addresses_data_in_row_major_order() {
        // Initialize terrain to be all plains
        let mut raw_terrain_data = [0; ROOM_AREA];

        // Adjust (1, 0) to be a swamp; in row-major order this is the second element
        // (index 1) in the array; in column-major order this is the 51st
        // element (index 50) in the array.
        raw_terrain_data[1] = 2; // Terrain::Swamp has the numeric representation 2

        // Construct the local terrain object
        let terrain = CompressedRoomTerrain::new_from_uncompressed_bits(&raw_terrain_data);

        // Pull the terrain for location (1, 0); if it comes out as a Swamp, then we
        // know the get_xy function pulls data in row-major order; if it comes
        // out as a Plain, then we know that it pulls in column-major order.
        let xy = unsafe { RoomXY::unchecked_new(1, 0) };
        let tile_type = terrain.get_xy(xy);
        assert_eq!(Terrain::Swamp, tile_type);
    }

    #[test]
    pub fn compressed_terrain_get_xy_matches_uncompressed_terrain() {
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

        // Grab the bits
        let bits = terrain.get_bits();

        // Build the new compressed terrain from the referenced bits
        let new_terrain = CompressedRoomTerrain::new_from_uncompressed_bits(bits);

        // Iterate over all room positions and verify that they match in both terrain
        // objects
        for x in 0..ROOM_SIZE {
            for y in 0..ROOM_SIZE {
                // Safety: x and y are both explicitly restricted to room size
                let xy = unsafe { RoomXY::unchecked_new(x, y) };
                assert_eq!(terrain.get_xy(xy), new_terrain.get_xy(xy));
            }
        }
    }

    #[test]
    pub fn compression_decompression_works_for_all_byte_combinations() {
        // Generate all combinatoric 4-tuples of terrain bit sequences
        let plains = 0b00u8;
        let walls = 0b01u8;
        let swamps = 0b10u8;
        let swampwalls = 0b11u8;

        let arr = [plains, walls, swamps, swampwalls];
        let iter = std::iter::repeat(arr).take(4).multi_cartesian_product();

        // Compress and decompress each combination, then assert that the decompressed data matches
        // the original data
        for a in iter {
            let compressed_byte = CompressedRoomTerrain::compress_4_bytes(&a);
            let decompressed_bytes = CompressedRoomTerrain::uncompress_byte(compressed_byte);
            assert_eq!(4, decompressed_bytes.len());
            for (o, d) in std::iter::zip(a, decompressed_bytes) {
                assert_eq!(o, d);
            }
        }
    }
}
