use std::mem::size_of;

use rle::{AppendRle, MergableSpan};

use screeps::{Terrain, LocalRoomTerrain, RoomXY, ROOM_AREA};
use screeps::local::{terrain_index_to_xy, xy_to_terrain_index};

use crate::compressed_terrain::compressed_terrain::CompressedRoomTerrain;

/// Specialized struct that encodes a run for [Terrain](screeps::Terrain), storing data in a bit-packed format.
#[derive(Clone)]
pub struct RoomTerrainPackedIndexedRLE {
    /// The compressed internal representation of the run data.
    ///
    /// Layout: 00ttssssssssssss
    /// From MSB to LSB:
    /// - The first two bits are always 0, and don't encode anything
    /// - The 3rd and 4th bits encode the terrain, and do not handle SwampWalls
    /// - The remaining 12 bits encode the RoomXY index; log2(2500) < 12
    packed: u16,
}

impl RoomTerrainPackedIndexedRLE {
    /// Creates a new run of the specified terrain type starting at the specified index in the
    /// room.
    ///
    /// Start is expected to be less than [ROOM_AREA](screeps::ROOM_AREA). It is recommended to use
    /// [xy_to_terrain_index](screeps::local::xy_to_terrain_index) to generate this value safely.
    pub fn new(terrain: Terrain, start: u16) -> Self {
        let packed = Self::get_packed_repr(terrain, start);
        Self::new_from_packed_repr(packed)
    }

    /// Creates a new run directly from the bit-packed internal representation.
    ///
    /// This is primarily useful for reconstituting the run when it's been serialized.
    pub fn new_from_packed_repr(packed: u16) -> Self {
        Self { packed }
    }

    /// Calculates the compressed internal representation of the provided run data.
    pub fn get_packed_repr(terrain: Terrain, start: u16) -> u16 {
        let terrain_bytes: u16 = match terrain {
            Terrain::Plain => 0,
            Terrain::Wall => 1,
            Terrain::Swamp => 2,
        };

        (terrain_bytes << 12) | (start)
    }

    /// The [Terrain] this run encodes.
    pub fn terrain(&self) -> Terrain {
        match self.packed >> 12 {
            0 => Terrain::Plain,
            1 => Terrain::Wall,
            2 => Terrain::Swamp,
            _ => unreachable!(),
        }
    }

    /// The linear terrain index that this run starts at.
    pub fn start(&self) -> u16 {
        self.packed & 0b111111111111 // Mask everything but the first 12 bits
    }

    /// Returns the compressed internal representation of this run.
    pub fn packed_repr(&self) -> u16 {
        self.packed
    }

    /// The amount of memory it takes to store this data.
    pub fn memory_size(&self) -> usize {
        size_of::<u16>()
    }
}

impl MergableSpan for RoomTerrainPackedIndexedRLE {
    fn can_append(&self, other: &Self) -> bool {
        // Since this is an indefinite-length run, we only need to check for start value orderings
        (self.terrain() == other.terrain()) & (self.start() <= other.start())
    }

    fn append(&mut self, other: Self) {
        // Appending the same token does nothing, since this just measures the start of the run
        ()
    }

    fn prepend(&mut self, other: Self) {
        // Unlike when appending, when prepending we do need to keep track of which run starts
        // sooner, since if the other run starts sooner, we need to extend this run back to that
        // one.
        if other.start() < self.start() {
            self.packed = other.packed; // This is equivalent to copying the start value, since the terrain values should already be the same
        }
    }
}

/// Encodes the terrain for a room in a run length encoded search tree.
///
/// O(lg(n)) search performance
pub struct BinarySearchPackedRoomTerrainRLE {
    vec: Vec<RoomTerrainPackedIndexedRLE>,
}

impl BinarySearchPackedRoomTerrainRLE {
    /// Creates a new, empty search tree.
    pub fn new() -> Self {
        Self {
            vec: Vec::new(),
        }
    }

    /// Appends an individual terrain run to the search tree.
    ///
    /// Returns true if the run was appended to the internal list, or false if the run was instead
    /// merged with the run at the end of the list.
    pub fn append_run(&mut self, run: RoomTerrainPackedIndexedRLE) -> bool {
        self.vec.push_rle(run)
    }

    /// Appends an individual terrain token to the search tree as a run.
    ///
    /// Returns true if the token-run was appended to the internal list, or false if the run was
    /// instead merged with the run at the end of the list.
    pub fn append_token(&mut self, terrain: Terrain, start: u16) -> bool {
        let run = RoomTerrainPackedIndexedRLE::new(terrain, start);
        self.append_run(run)
    }

    /// Searches for the terrain type at the tile given by the linear terrain index.
    ///
    /// Returns None if:
    /// - There are no runs in the search tree.
    /// - The requested tile index is before the start of the first run.
    ///
    /// Otherwise, this returns the [Terrain] for the tile requested.
    pub fn find_token_at_index(&self, index: u16) -> Option<Terrain> {
        if self.vec.len() == 0 {
            None
        } else {
            // Edge case: If the token index requested is before the first run, return None and
            // avoid a fruitless search
            if index < self.vec[0].start() {
                None
            } else {
                // Slices already implement binary search, so we can avoid all the manual implementation
                let idx = (&self.vec).partition_point(|item| item.start() < index);

                let run_idx = if idx == self.vec.len() {
                    // If the token index requested is after the start of the last run, the partition point can
                    // return self.vec.len() as the run index
                    idx - 1
                } else {
                    // Two cases:
                    // - The token index is at the start of a run; this means we want the current
                    // run that partition point gave us
                    // - The token index is in the middle of a run; this means we want the previous
                    // run from what `partition_point` gave us
                    let current_run = &self.vec[idx];
                    if current_run.start() == index {
                        idx
                    } else {
                        idx - 1
                    }
                };

                Some(self.vec[run_idx].terrain())
            }
        }
    }

    /// Returns the number of runs in the search tree.
    pub fn num_runs(&self) -> usize {
        self.vec.len()
    }

    /// Returns the token of the last run in the search tree.
    ///
    /// Returns None if the search tree is empty.
    pub fn last_token(&self) -> Option<Terrain> {
        if self.vec.len() > 0 {
            Some(self.vec[self.vec.len()].terrain())
        } else {
            None
        }
    }

    /// The amount of memory it takes to store this data.
    pub fn memory_size(&self) -> usize {
        let data_size = if self.vec.len() > 0 {
            self.vec.len() * self.vec[0].memory_size()
        } else {
            0
        };

        let vec_size = size_of::<Vec<RoomTerrainPackedIndexedRLE>>();

        data_size + vec_size
    }
}

/// User-friendly interface for getting terrain data.
///
/// Uses [BinarySearchPackedRoomTerrainRLE] internally to store data efficiently.
pub struct PackedRLERoomTerrain {
    data: BinarySearchPackedRoomTerrainRLE,
}

impl PackedRLERoomTerrain {
    /// Converts uncompressed room terrain data into a RLE-compressed format.
    pub fn new_from_uncompressed_terrain(terrain: &LocalRoomTerrain) -> Self {
        let mut data = BinarySearchPackedRoomTerrainRLE::new();

        for idx in 0..ROOM_AREA {
            let xy = terrain_index_to_xy(idx);
            let tile = terrain.get_xy(xy);
            data.append_token(tile, idx as u16);
        }

        Self { data }
    }

    /// Converts bit-packed compressed terrain into a RLE-compressed format.
    pub fn new_from_compressed_terrain(terrain: &CompressedRoomTerrain) -> Self {
        let mut data = BinarySearchPackedRoomTerrainRLE::new();

        for idx in 0..ROOM_AREA {
            let xy = terrain_index_to_xy(idx);
            let tile = terrain.get_xy(xy);
            data.append_token(tile, idx as u16);
        }

        Self { data }
    }

    /// Gets the terrain value for the specified tile.
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
    pub fn packed_rle_stores_data_correctly() {
        for terrain in vec!(Terrain::Plain, Terrain::Swamp, Terrain::Wall) {
            for i in 0..ROOM_AREA {
                let rle = RoomTerrainPackedIndexedRLE::new(terrain, i as u16);
                println!("Terrain: {:?}, Start: {}, Packed Repr: {:b}", terrain, i, rle.packed_repr());
                assert_eq!(rle.terrain(), terrain, "Terrain not retrieved correctly");
                assert_eq!(rle.start(), i as u16, "Start index not retrieved correctly");
            }
        }
    }

    #[test]
    pub fn packed_rle_terrain_addresses_data_in_row_major_order() {
        // Initialize terrain to be all plains
        let mut raw_terrain_data = [0; ROOM_AREA];

        // Adjust (1, 0) to be a swamp; in row-major order this is the second element
        // (index 1) in the array; in column-major order this is the 51st
        // element (index 50) in the array.
        raw_terrain_data[1] = 2; // Terrain::Swamp has the numeric representation 2

        // Construct the local terrain object
        let compressed_terrain = CompressedRoomTerrain::new_from_uncompressed_bits(&raw_terrain_data);

        let terrain = PackedRLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);

        // Pull the terrain for location (1, 0); if it comes out as a Swamp, then we
        // know the get_xy function pulls data in row-major order; if it comes
        // out as a Plain, then we know that it pulls in column-major order.
        let xy = unsafe { RoomXY::unchecked_new(1, 0) };
        let tile_type = terrain.get_xy(xy);
        assert_eq!(Terrain::Swamp, tile_type, "Terrain mismatch at {xy}");
    }

    #[test]
    pub fn packed_rle_terrain_get_xy_matches_uncompressed_terrain() {
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
        let new_terrain = PackedRLERoomTerrain::new_from_uncompressed_terrain(&terrain);

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

    #[test]
    pub fn room_terrain_packed_indexed_rle_can_append_accepts_valid_runs() {
        let max_start: u16 = 1000;
        for start in 0..=max_start {
            for after in (start + 1)..=(max_start + 1) {
                let lower_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Plain, start);
                let higher_matching_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Plain, after);
                let higher_nonmatching_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Wall, after);

                assert!(lower_rle.can_append(&lower_rle), "Valid self append failed: {start} = {after}");
                assert!(lower_rle.can_append(&higher_matching_rle), "Valid append failed: {start} < {after}");
                assert!(!lower_rle.can_append(&higher_nonmatching_rle), "Non-matching token append succeeded");
                assert!(!higher_matching_rle.can_append(&lower_rle), "Out-of-order matching token append succeeded: {start} < {after}");
                assert!(!higher_nonmatching_rle.can_append(&lower_rle), "Out-of-order non-matching token append succeeded");
            }
        }
    }

    #[test]
    pub fn binary_search_packed_room_terrain_rle_append_run_merges_properly() {
        let mut rle_data = BinarySearchPackedRoomTerrainRLE::new();

        let start = 10;
        let after = 20;

        let lower_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Plain, start);
        let higher_matching_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Plain, after);
        let higher_nonmatching_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Wall, after);

        assert_eq!(rle_data.num_runs(), 0); // There should be no runs before we've started anything

        // Add the initial run
        rle_data.append_run(lower_rle);
        assert_eq!(rle_data.num_runs(), 1);

        // Test that appending a matching token run doesn't increase the length of the internal
        // data vector
        rle_data.append_run(higher_matching_rle);
        assert_eq!(rle_data.num_runs(), 1); // We appended a matching run, so there should not be an increase in the total number of runs

        // Test that appending a non-matching token run increases the length of the internal data
        // vector
        rle_data.append_run(higher_nonmatching_rle);
        assert_eq!(rle_data.num_runs(), 2); // We appended a non-matching run, so there should be an increase in the total number of runs
    }

    #[test]
    pub fn binary_search_packed_room_terrain_rle_append_terrain_merges_properly() {
        let mut rle_data = BinarySearchPackedRoomTerrainRLE::new();

        let start = 10;
        let after = 20;

        let matching_token = Terrain::Plain;
        let nonmatching_token = Terrain::Wall;

        let lower_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Plain, start);

        assert_eq!(rle_data.num_runs(), 0); // There should be no runs before we've started anything

        // Add the initial run
        rle_data.append_run(lower_rle);
        assert_eq!(rle_data.num_runs(), 1);

        // Test that appending a matching token doesn't increase the length of the internal
        // data vector
        rle_data.append_token(matching_token, after);
        assert_eq!(rle_data.num_runs(), 1); // We appended a matching token, so there should not be an increase in the total number of runs

        // Test that appending a non-matching token increases the length of the internal data
        // vector
        rle_data.append_token(nonmatching_token, after);
        assert_eq!(rle_data.num_runs(), 2); // We appended a non-matching token, so there should be an increase in the total number of runs
    }

    #[test]
    pub fn binary_search_packed_room_terrain_rle_find_token_at_index_returns_none_when_empty() {
        let mut rle_data = BinarySearchPackedRoomTerrainRLE::new();
        assert_eq!(rle_data.find_token_at_index(0), None);
    }

    #[test]
    pub fn binary_search_packed_room_terrain_rle_find_token_at_index_returns_none_for_index_before_first_run() {
        let mut rle_data = BinarySearchPackedRoomTerrainRLE::new();
        let rle = RoomTerrainPackedIndexedRLE::new(Terrain::Plain, 10);
        rle_data.append_run(rle);
        assert_eq!(rle_data.num_runs(), 1);
        assert_eq!(rle_data.find_token_at_index(5), None);
    }

    #[test]
    pub fn binary_search_packed_room_terrain_rle_find_token_at_index_works() {
        let mut rle_data = BinarySearchPackedRoomTerrainRLE::new();

        let (first_start, first_len) = (10, 4);
        let (second_start, second_len) = ((first_start + first_len), 13);
        let (third_start, third_len) = ((second_start + second_len), 2);
        let (fourth_start, fourth_len) = ((third_start + third_len), 1);
        let (fifth_start, fifth_len) = ((fourth_start + fourth_len), 21);

        let first_end_inclusive = second_start - 1;
        let second_end_inclusive = third_start - 1;
        let third_end_inclusive = fourth_start - 1;
        let fourth_end_inclusive = fifth_start - 1;
        let fifth_end_inclusive = fifth_start + fifth_len;

        let first_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Plain, first_start);
        let second_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Wall, second_start);
        let third_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Plain, third_start);
        let fourth_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Wall, fourth_start);
        let fifth_rle = RoomTerrainPackedIndexedRLE::new(Terrain::Plain, fifth_start);

        let true_token_ranges: Vec<(u16, u16)> = vec!((first_start, second_start), (third_start, fourth_start), (fifth_start, fifth_end_inclusive+1));
        let false_token_ranges: Vec<(u16, u16)> = vec!((second_start, third_start), (fourth_start, fifth_start));

        println!("First range inclusive: {first_start} - {first_end_inclusive}");
        println!("Second range inclusive: {second_start} - {second_end_inclusive}");
        println!("Third range inclusive: {third_start} - {third_end_inclusive}");
        println!("Fourth range inclusive: {fourth_start} - {fourth_end_inclusive}");
        println!("Fifth range inclusive: {fifth_start} - {fifth_end_inclusive}");

        assert_eq!(rle_data.num_runs(), 0); // There should be no runs before we've started anything

        // Add the runs
        rle_data.append_run(first_rle);
        rle_data.append_run(second_rle);
        rle_data.append_run(third_rle);
        rle_data.append_run(fourth_rle);
        rle_data.append_run(fifth_rle);
        assert_eq!(rle_data.num_runs(), 5);

        // Validate that the known token ranges match
        for (range_start, range_end) in true_token_ranges {
            for token_index in range_start..range_end {
                assert_eq!(rle_data.find_token_at_index(token_index), Some(Terrain::Plain), "Token index {token_index} expected to be true");
            }
        }

        for (range_start, range_end) in false_token_ranges {
            for token_index in range_start..range_end {
                assert_eq!(rle_data.find_token_at_index(token_index), Some(Terrain::Wall), "Token index {token_index} expected to be false");
            }
        }
    }
}
