use screeps::{ExitDirection, Terrain, RoomName};

use crate::compressed_terrain::compressed_room_edge_terrain::RoomEdgeTerrain;


/// Compact representation of an entire exit along a room edge.
///
/// Note: Storing collections of these will not be as efficient as just storing the raw edge
/// terrain. This structure should be used for when you need to work with and reason about the exit
/// properties, not for when you need to store all of the exits on an edge. For storing all the
/// exit data in a compact representation, see [RoomExitsData].
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct RoomExit {
    /// The packed representation of this exit, comprised of a start position and a length, as well
    /// as an exit direction. The position and length both require 6 bits to store, and the exit
    /// direction requires 3 bits to store, thus all three can be encoded with a single u16.
    ///
    /// Format:
    /// 0LLLLLLDDDPPPPPP
    ///
    /// This allows us to:
    /// - Get the start coordinate of the exit with a single bitmask operation
    /// - Get the length with a single bitshift operation
    /// - Get the exit direction with a combination bitmask and bitshift
    ///
    /// The first two are single operations, and thus as fast as possible, while the third is 2
    /// operations, slightly slower, but still extremely fast. This decision is because we're far
    /// more likely to want to get the start and length values than we are the exit direction, so
    /// we should optimize our representation to make those the faster operations.
    packed: u16,
}

impl RoomExit {
    const EXIT_DIRECTION_OFFSET: u16 = 6;
    const LENGTH_OFFSET: u16 = 9;
    const START_POSITION_BITMASK: u16 = 0b111111;
    const EXIT_DIRECTION_BITMASK: u16 = 0b111000000;
    const EXIT_DIRECTION_INVERTED_BITMASK: u16 = 0b111111000111111;

    /// Creates a new RoomExit from the packed representation.
    ///
    /// Note: This will convert the exit direction to ExitDirection::Top if the relevant bits are
    /// not a valid ExitDirection.
    pub fn new_from_packed(packed: u16) -> Self {
        // Safety: Validate the exit direction bits are valid
        let dir_bits = (packed & Self::EXIT_DIRECTION_BITMASK) >> Self::EXIT_DIRECTION_OFFSET;
        let final_packed = match dir_bits {
            1 | 3 | 5 | 7 => packed,
            _ => (packed & Self::EXIT_DIRECTION_INVERTED_BITMASK) | ((ExitDirection::Top as u16) << Self::EXIT_DIRECTION_OFFSET),
        };

        Self { packed: final_packed }
    }

    /// Creates a new RoomExit from the start and length parameters.
    pub fn new(start: u8, length: u8, direction: ExitDirection) -> Self {
        let packed = Self::get_packed_from_parameters(start, length, direction);

        Self { packed }
    }

    /// Helper function to get the packed representation from the start and length parameters.
    pub fn get_packed_from_parameters(start: u8, length: u8, direction: ExitDirection) -> u16 {
        let direction_val = direction as u16;
        ((length as u16) << Self::LENGTH_OFFSET) | (direction_val << Self::EXIT_DIRECTION_OFFSET) | start as u16
    }

    /// The start position of this exit.
    pub fn start(&self) -> u8 {
        // Safety: This is safe to convert, because we're masking out all but the first 6 bits,
        // which is 2 bits below the u8 limit
        (self.packed & Self::START_POSITION_BITMASK) as u8
    }

    /// The length of this exit.
    pub fn len(&self) -> u8 {
        // Safety: This is safe to convert, because we're shifting down the most-significant 10
        // bits, but the most significant 4 bits are always 0, meaning we only have 6 bits of
        // actual data, which is below the u8 limit
        (self.packed >> Self::LENGTH_OFFSET) as u8
    }

    /// The end position of this exit.
    pub fn end(&self) -> u8 {
        self.start().saturating_add(self.len()).saturating_sub(1)
    }

    /// The edge that this exit is on.
    pub fn exit_direction(&self) -> ExitDirection {
        let val = (self.packed & Self::EXIT_DIRECTION_BITMASK) >> Self::EXIT_DIRECTION_OFFSET;
        match val {
            1 => ExitDirection::Top,
            3 => ExitDirection::Right,
            5 => ExitDirection::Bottom,
            7 => ExitDirection::Left,

            // Safety: We're only extracting values that we set; they shouldn't ever be invalid
            _ => unreachable!(),
        }
    }

    /// The packed representation of this exit.
    pub fn packed(&self) -> u16 {
        self.packed
    }

    /// How much space this exit takes up in memory (in bytes).
    pub fn memory_size(&self) -> usize {
        std::mem::size_of::<u16>()
    }

    /// Extracts the individual exits for each edge from the compressed room edge terrain.
    ///
    /// Returned ordering is: Top, Right, Bottom, Left
    pub fn get_exits_from_edge_terrain(terrain: &RoomEdgeTerrain) -> (Vec<Self>, Vec<Self>, Vec<Self>, Vec<Self>) {
        let top_terrain = terrain.get_top_edge_terrain();
        let right_terrain = terrain.get_right_edge_terrain();
        let bottom_terrain = terrain.get_bottom_edge_terrain();
        let left_terrain = terrain.get_left_edge_terrain();

        let top_exits = Self::get_exits_from_single_edge(&top_terrain, ExitDirection::Top);
        let right_exits = Self::get_exits_from_single_edge(&right_terrain, ExitDirection::Right);
        let bottom_exits = Self::get_exits_from_single_edge(&bottom_terrain, ExitDirection::Bottom);
        let left_exits = Self::get_exits_from_single_edge(&left_terrain, ExitDirection::Left);

        (top_exits, right_exits, bottom_exits, left_exits)
    }

    /// Utility function that processes edge terrain into a list of exits.
    ///
    /// Returned vector can be empty if the edge is entirely Walls, and thus has no exits.
    pub fn get_exits_from_single_edge(terrain: &[Terrain; 50], direction: ExitDirection) -> Vec<Self> {
        let mut exits = Vec::new();

        // These are how we track the current exit that we're processing;
        // - length will always be non-zero if we're currently processing an exit, and gets reset to
        //   0 once the exit is finalized and pushed onto the output vector
        // - start can be any value from 0 to 49; on MMO it won't ever be 0 or 49, but if we're
        //   using raw terrain data, it can happen
        let mut current_exit_start = 0;
        let mut current_exit_length = 0;

        for i in 0..50 {
            if terrain[i] == Terrain::Wall {
                // If we've hit a wall, then if we were previously tracking an exit, it's done and
                // we need to store it
                if current_exit_length > 0 {
                    let exit = Self::new(current_exit_start, current_exit_length, direction);
                    exits.push(exit);
                    current_exit_start = 0;
                    current_exit_length = 0;
                }

                continue;
            }

            // At this point, we know we're on a exit, so we need to determine if we're on the
            // first tile of the exit or not
            let is_new_exit = {
                if i == 0 {
                    // If we're on the first tile, then it's guaranteed to be a new exit
                    true
                } else {
                    // If the previous tile was a Wall, then this tile is the start of a new exit
                    terrain[i-1] == Terrain::Wall
                }
            };

            // If we're on a new exit, adjust our tracking variables appropriately
            if is_new_exit {
                current_exit_start = i as u8;
                current_exit_length = 1;
            } else {
                current_exit_length += 1;
            }
        }

        // Catch a final exit that ends on the 50th tile; this won't happen on MMO, but it could
        // happen theoretically with raw edge terrain.
        if current_exit_length > 0 {
            let exit = Self::new(current_exit_start, current_exit_length, direction);
            exits.push(exit);
        }

        exits
    }
}

/// Compactly stores information about all the exits in a room.
pub struct RoomExitsData {
    /// Unfortunately, there really isn't any way to store this better than just 24 raw bytes of
    /// compressed edge data.
    data: RoomEdgeTerrain,

    room: RoomName,
    num_top_exits: usize,
    num_right_exits: usize,
    num_bottom_exits: usize,
    num_left_exits: usize,
}

impl RoomExitsData {
    pub fn new_from_compressed_edge_terrain_data(data: RoomEdgeTerrain, room: RoomName) -> Self {
        let num_top_exits = RoomExit::get_exits_from_single_edge(&data.get_top_edge_terrain(), ExitDirection::Top).len();
        let num_right_exits = RoomExit::get_exits_from_single_edge(&data.get_right_edge_terrain(), ExitDirection::Right).len();
        let num_bottom_exits = RoomExit::get_exits_from_single_edge(&data.get_bottom_edge_terrain(), ExitDirection::Bottom).len();
        let num_left_exits = RoomExit::get_exits_from_single_edge(&data.get_left_edge_terrain(), ExitDirection::Left).len();

        Self {
            data,
            room,
            num_top_exits,
            num_right_exits,
            num_bottom_exits,
            num_left_exits,
        }
    }

    /// The amount of memory used to store this data, in bytes.
    pub fn memory_size(&self) -> usize {
        self.data.memory_size()
    }

    /// The exits, if any, along the top edge of the room.
    pub fn top_edge_exits(&self) -> Vec<RoomExit> {
        RoomExit::get_exits_from_single_edge(&self.data.get_top_edge_terrain(), ExitDirection::Top)
    }

    /// The exits, if any, along the right edge of the room.
    pub fn right_edge_exits(&self) -> Vec<RoomExit> {
        RoomExit::get_exits_from_single_edge(&self.data.get_right_edge_terrain(), ExitDirection::Right)
    }

    /// The exits, if any, along the bottom edge of the room.
    pub fn bottom_edge_exits(&self) -> Vec<RoomExit> {
        RoomExit::get_exits_from_single_edge(&self.data.get_bottom_edge_terrain(), ExitDirection::Bottom)
    }

    /// The exits, if any, along the left edge of the room.
    pub fn left_edge_exits(&self) -> Vec<RoomExit> {
        RoomExit::get_exits_from_single_edge(&self.data.get_left_edge_terrain(), ExitDirection::Left)
    }

    /// The number of exits along the top edge of the room.
    ///
    /// This is more efficient than constructing all of the exits, if you just need the exit count.
    pub fn num_top_exits(&self) -> usize {
        self.num_top_exits
    }

    /// The number of exits along the right edge of the room.
    ///
    /// This is more efficient than constructing all of the exits, if you just need the exit count.
    pub fn num_right_exits(&self) -> usize {
        self.num_right_exits
    }

    /// The number of exits along the bottom edge of the room.
    ///
    /// This is more efficient than constructing all of the exits, if you just need the exit count.
    pub fn num_bottom_exits(&self) -> usize {
        self.num_bottom_exits
    }

    /// The number of exits along the left edge of the room.
    ///
    /// This is more efficient than constructing all of the exits, if you just need the exit count.
    pub fn num_left_exits(&self) -> usize {
        self.num_left_exits
    }

    /// A reference to the underlying edge terrain data for the room.
    pub fn edge_terrain_data(&self) -> &RoomEdgeTerrain {
        &self.data
    }

    /// Returns true if the top edge has exits and has a neighbor to the north, false otherwise.
    ///
    /// This is more efficient than `self.top_edge_exits().len()` if you're just wanting
    /// connectivity data, as it doesn't create the exits themselves, just checks for their
    /// existence.
    pub fn connected_to_top_neighbor(&self) -> bool {
        (self.num_top_exits > 0) && top_room(self.room).is_some()
    }

    /// Returns true if the right edge has exits and has a neighbor to the east, false otherwise.
    ///
    /// This is more efficient than `self.right_edge_exits().len()` if you're just wanting
    /// connectivity data, as it doesn't create the exits themselves, just checks for their
    /// existence.
    pub fn connected_to_right_neighbor(&self) -> bool {
        (self.num_right_exits > 0) && right_room(self.room).is_some()
    }

    /// Returns true if the bottom edge has exits and has a neighbor to the south, false otherwise.
    ///
    /// This is more efficient than `self.bottom_edge_exits().len()` if you're just wanting
    /// connectivity data, as it doesn't create the exits themselves, just checks for their
    /// existence.
    pub fn connected_to_bottom_neighbor(&self) -> bool {
        (self.num_bottom_exits > 0) && bottom_room(self.room).is_some()
    }

    /// Returns true if the left edge has exits and has a neighbor to the west, false otherwise.
    ///
    /// This is more efficient than `self.left_edge_exits().len()` if you're just wanting
    /// connectivity data, as it doesn't create the exits themselves, just checks for their
    /// existence.
    pub fn connected_to_left_neighbor(&self) -> bool {
        (self.num_left_exits > 0) && left_room(self.room).is_some()
    }

    /// Returns the room exit identified by iterating through edges clockwise (top, right, bottom,
    /// left) and then scanning through each edge linearly (left-to-right, top-to-bottom).
    ///
    /// Example:
    /// If a room has no exits along the top edge, but has 2 exits along the right edge, then the
    /// exit at index 0 would be the exit on the right edge with the lower start position, and the
    /// exit at index 1 would be the exit on the right edge with the higher start position.
    ///
    /// Returns None if the index represents a non-existent exit; this can happen if:
    /// - There are no exits to the room
    /// - There are exits to the room, but the index is greater than the number of exits - 1 (since
    ///   we use a zero-based index)
    pub fn get_exit_by_index(&self, index: usize) -> Option<RoomExit> {
        let total_num_exits = self.num_top_exits + self.num_right_exits + self.num_bottom_exits + self.num_left_exits;
        if total_num_exits == 0 {
            // If there aren't any exits, no index is valid
            None
        } else {
            let max_index = total_num_exits - 1; // Safety: We know total_num_exits is greater than 0, so this will never underflow
            if index > max_index {
                // Index references an exit that doesn't exist
                None
            } else {
                // The index is valid, find the exit

                let min_idx_top = 0;
                let max_idx_top = if self.num_top_exits > 0 {
                    min_idx_top + self.num_top_exits - 1
                } else {
                    min_idx_top
                };

                let min_idx_right = if self.num_top_exits > 0 {
                    max_idx_top + 1
                } else {
                    max_idx_top
                };
                let max_idx_right = if self.num_right_exits > 0 {
                    min_idx_right + self.num_right_exits - 1
                } else {
                    min_idx_right
                };

                let min_idx_bottom = if self.num_right_exits > 0 {
                    max_idx_right + 1
                } else {
                    max_idx_right
                };
                let max_idx_bottom = if self.num_bottom_exits > 0 {
                    min_idx_bottom + self.num_bottom_exits - 1
                } else {
                    min_idx_bottom
                };

                let min_idx_left = if self.num_bottom_exits > 0 {
                    max_idx_bottom + 1
                } else {
                    max_idx_bottom
                };
                let max_idx_left = if self.num_left_exits > 0 {
                    min_idx_left + self.num_left_exits - 1
                } else {
                    min_idx_left
                };

                // - If index is greater than the top edge max index, continue onward
                // - If the index is less than the top edge max index, generate the exits and
                //   extract the correct one
                if self.num_top_exits > 0 {
                    if index <= max_idx_top {
                        // Index is one of these exits, generate them and return the correct one
                        let exits = self.top_edge_exits();
                        let local_index = index - min_idx_top;
                        return exits.get(local_index).copied();
                    }
                }

                // - If index is greater than the right edge max index, continue onward
                // - If the index is less than the right edge max index, generate the exits and
                //   extract the correct one
                if self.num_right_exits > 0 {
                    if index <= max_idx_right {
                        // Index is one of these exits, generate them and return the correct one
                        let exits = self.right_edge_exits();
                        let local_index = index - min_idx_right;
                        return exits.get(local_index).copied();
                    }
                }

                // - If index is greater than the bottom edge max index, continue onward
                // - If the index is less than the bottom edge max index, generate the exits and
                //   extract the correct one
                if self.num_bottom_exits > 0 {
                    if index <= max_idx_bottom {
                        // Index is one of these exits, generate them and return the correct one
                        let exits = self.bottom_edge_exits();
                        let local_index = index - min_idx_bottom;
                        return exits.get(local_index).copied();
                    }
                }

                // - If index is greater than the left edge max index, return None (shouldn't ever
                //   happen, but edge cases are a PITA)
                // - If the index is less than the left edge max index, generate the exits and
                //   extract the correct one
                if self.num_left_exits > 0 {
                    if index <= max_idx_left {
                        // Index is one of these exits, generate them and return the correct one
                        let exits = self.left_edge_exits();
                        let local_index = index - min_idx_left;
                        return exits.get(local_index).copied();
                    }
                }

                // We should never get here due to pre-checks, but just in case, return None since
                // we couldn't find the proper exit
                None
            }
        }
    }

    /// The room this data is for.
    pub fn room(&self) -> RoomName {
        self.room
    }
}

/// Utility function to return the room above the given room, if it exists.
pub fn top_room(room: RoomName) -> Option<RoomName> {
    room.checked_add((0, -1))
}

/// Utility function to return the room to the right of the given room, if it exists.
pub fn right_room(room: RoomName) -> Option<RoomName> {
    room.checked_add((1, 0))
}

/// Utility function to return the room below the given room, if it exists.
pub fn bottom_room(room: RoomName) -> Option<RoomName> {
    room.checked_add((0, 1))
}

/// Utility function to return the room to the left of the given room, if it exists.
pub fn left_room(room: RoomName) -> Option<RoomName> {
    room.checked_add((-1, 0))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn room_exit_verify_parameter_packing() {
        // This is not precisely conformant to MMO, since 0 and 49 are both always Walls. However,
        // the math should still work out, and this makes the data structure more resilient.
        let directions = [ExitDirection::Top, ExitDirection::Right, ExitDirection::Bottom, ExitDirection::Left];
        for start in 0..50 {
            let max_length = 50 - start;
            for length in 1..max_length {
                for direction in directions {
                    let exit = RoomExit::new(start, length, direction);

                    let packed = exit.packed();

                    let end = start + length - 1; // Since we restrict start and length, this should always be in the range [0, 49], with no over or underflows

                    assert_eq!(exit.start(), start, "Start position unpacking mismatch, ({start}, {length}, {direction:?}), Packed: {packed:b}");
                    assert_eq!(exit.len(), length, "Length unpacking mismatch, ({start}, {length}, {direction:?}), Packed: {packed:b}");
                    assert_eq!(exit.end(), end, "End position calculation mismatch, ({start}, {length}, {end}, {direction:?})");
                    assert_eq!(exit.exit_direction(), direction, "Exit direction calculation mismatch, ({start}, {length}, {direction:?}, Packed: {packed:b})");
                }
            }
        }
    }

    #[test]
    pub fn room_exit_new_from_packed_matches_original_data() {
        let directions = [ExitDirection::Top, ExitDirection::Right, ExitDirection::Bottom, ExitDirection::Left];
        for start in 0..50 {
            let max_length = 50 - start;
            for length in 1..max_length {
                for direction in directions {
                    let exit = RoomExit::new(start, length, direction);

                    let packed = exit.packed();

                    let new_exit = RoomExit::new_from_packed(packed);

                    let end = start + length - 1; // Since we restrict start and length, this should always be in the range [0, 49], with no over or underflows

                    assert_eq!(exit.start(), new_exit.start(), "Start position mismatch, ({start}, {length}, {direction:?}), Packed: {packed:b}");
                    assert_eq!(exit.len(), new_exit.len(), "Length mismatch, ({start}, {length}, {direction:?}), Packed: {packed:b}");
                    assert_eq!(exit.end(), new_exit.end(), "End position mismatch, ({start}, {length}, {end}, {direction:?})");
                    assert_eq!(exit.exit_direction(), new_exit.exit_direction(), "Exit direction mismatch, ({start}, {length}, {direction:?}, Packed: {packed:b})");
                }
            }
        }
    }

    #[test]
    pub fn room_exit_new_from_packed_sanitizes_invalid_direction_data() {
        let directions = [ExitDirection::Top, ExitDirection::Right, ExitDirection::Bottom, ExitDirection::Left];
        let invalid_direction_values: [u16; 4] = [0, 2, 4, 6]; // These are the only invalid values that can fit in 3 bits
        let direction_include_bitmask = 0b111111000111111;
        for start in 0..50 {
            let max_length = 50 - start;
            for length in 1..max_length {
                for direction in directions {
                    for invalid_direction_val in &invalid_direction_values {
                        let exit = RoomExit::new(start, length, direction);

                        let packed = exit.packed();

                        let invalid_packed = (packed & direction_include_bitmask) | (invalid_direction_val << RoomExit::EXIT_DIRECTION_OFFSET);

                        let new_exit = RoomExit::new_from_packed(invalid_packed);

                        let new_packed = new_exit.packed();

                        let sanitized_dir_bits = (new_packed & RoomExit::EXIT_DIRECTION_BITMASK) >> RoomExit::EXIT_DIRECTION_OFFSET;

                        let end = start + length - 1; // Since we restrict start and length, this should always be in the range [0, 49], with no over or underflows

                        assert_eq!(exit.start(), new_exit.start(), "Start position mismatch, ({start}, {length}, {direction:?}), Packed: {new_packed:b}");
                        assert_eq!(exit.len(), new_exit.len(), "Length mismatch, ({start}, {length}, {direction:?}), Packed: {new_packed:b}");
                        assert_eq!(exit.end(), new_exit.end(), "End position mismatch, ({start}, {length}, {end}, {direction:?})");
                        assert_eq!(sanitized_dir_bits, ExitDirection::Top as u16, "Exit direction not sanitized, ({start}, {length}, {direction:?}, Packed: {new_packed:b}, Invalid Packed: {invalid_packed:b})");
                        assert_eq!(new_exit.exit_direction(), ExitDirection::Top, "Exit direction not sanitized, ({start}, {length}, {direction:?}, Packed: {new_packed:b})");
                    }
                }
            }
        }
    }

    #[test]
    pub fn room_exit_get_exits_from_single_edge_returns_empty_for_all_walls() {
        let terrain = [Terrain::Wall; 50];

        let exits = RoomExit::get_exits_from_single_edge(&terrain, ExitDirection::Top);

        assert_eq!(exits.len(), 0);
    }

    #[test]
    pub fn room_exit_get_exits_from_single_edge_returns_expected_number_of_exits() {
        // Single giant exit; e.g. highways, crossroads
        let mut terrain = [Terrain::Plain; 50];
        terrain[0] = Terrain::Wall;
        terrain[49] = Terrain::Wall;

        let exits = RoomExit::get_exits_from_single_edge(&terrain, ExitDirection::Top);

        assert_eq!(exits.len(), 1);

        // Two exits
        let mut terrain = [Terrain::Plain; 50];
        terrain[0] = Terrain::Wall;
        terrain[49] = Terrain::Wall;

        for i in 5..10 {
            terrain[i] = Terrain::Wall;
        }

        let exits = RoomExit::get_exits_from_single_edge(&terrain, ExitDirection::Top);

        assert_eq!(exits.len(), 2);

        // Three exits
        let mut terrain = [Terrain::Plain; 50];
        terrain[0] = Terrain::Wall;
        terrain[49] = Terrain::Wall;

        for i in 5..10 {
            terrain[i] = Terrain::Wall;
        }

        for i in 23..25 {
            terrain[i] = Terrain::Wall;
        }

        let exits = RoomExit::get_exits_from_single_edge(&terrain, ExitDirection::Top);

        assert_eq!(exits.len(), 3);
    }

    #[test]
    pub fn room_exits_data_get_exit_by_index_returns_none_for_bad_indices() {
        let room_name = RoomName::new("W0N0").unwrap();

        // Test cases
        // - No exits at all
        let edge = [Terrain::Wall; 50];
        let terrain = RoomEdgeTerrain::new_from_terrain_slices(&edge, &edge, &edge, &edge).unwrap();
        let exits_data = RoomExitsData::new_from_compressed_edge_terrain_data(terrain, room_name);
        assert_eq!(exits_data.get_exit_by_index(0), None, "Exit returned when none exists");

        // - Index larger than max possible index for num exits
        let edge = [Terrain::Plain; 50];
        let terrain = RoomEdgeTerrain::new_from_terrain_slices(&edge, &edge, &edge, &edge).unwrap();
        let exits_data = RoomExitsData::new_from_compressed_edge_terrain_data(terrain, room_name);
        assert_eq!(exits_data.get_exit_by_index(8), None, "Exit returned for invalid index");
    }

    #[test]
    pub fn room_exits_data_get_exit_by_index_returns_some_for_exits_on_4_edges() {
        let room_name = RoomName::new("W0N0").unwrap();

        let edge = [Terrain::Plain; 50];
        let terrain = RoomEdgeTerrain::new_from_terrain_slices(&edge, &edge, &edge, &edge).unwrap();
        let exits_data = RoomExitsData::new_from_compressed_edge_terrain_data(terrain, room_name);

        assert_eq!(1, exits_data.num_top_exits(), "Top exit count mismatch");
        assert_eq!(1, exits_data.num_right_exits(), "Right exit count mismatch");
        assert_eq!(1, exits_data.num_bottom_exits(), "Bottom exit count mismatch");
        assert_eq!(1, exits_data.num_left_exits(), "Left exit count mismatch");

        // -- Top edge
        let exit_opt = exits_data.get_exit_by_index(0);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Top, "Exit direction invalid");

        // -- Right edge
        let exit_opt = exits_data.get_exit_by_index(1);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Right, "Exit direction invalid");

        // -- Bottom edge
        let exit_opt = exits_data.get_exit_by_index(2);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Bottom, "Exit direction invalid");

        // -- Left edge
        let exit_opt = exits_data.get_exit_by_index(3);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Left, "Exit direction invalid");
    }

    #[test]
    pub fn room_exits_data_get_exit_by_index_returns_some_for_exits_except_top_edge() {
        let room_name = RoomName::new("W0N0").unwrap();

        let wall_edge = [Terrain::Wall; 50];
        let edge = [Terrain::Plain; 50];
        let terrain = RoomEdgeTerrain::new_from_terrain_slices(&wall_edge, &edge, &edge, &edge).unwrap();
        let exits_data = RoomExitsData::new_from_compressed_edge_terrain_data(terrain, room_name);

        assert_eq!(0, exits_data.num_top_exits(), "Top exit count mismatch");
        assert_eq!(1, exits_data.num_right_exits(), "Right exit count mismatch");
        assert_eq!(1, exits_data.num_bottom_exits(), "Bottom exit count mismatch");
        assert_eq!(1, exits_data.num_left_exits(), "Left exit count mismatch");

        // -- First exit should be the right edge exit
        let exit_opt = exits_data.get_exit_by_index(0);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Right, "Exit direction invalid");

        // -- Second exit should be the bottom edge exit
        let exit_opt = exits_data.get_exit_by_index(1);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Bottom, "Exit direction invalid");

        // -- Third exit should be the left edge exit
        let exit_opt = exits_data.get_exit_by_index(2);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Left, "Exit direction invalid");
    }

    #[test]
    pub fn room_exits_data_get_exit_by_index_returns_some_for_exits_except_right_edge() {
        let room_name = RoomName::new("W0N0").unwrap();

        let wall_edge = [Terrain::Wall; 50];
        let edge = [Terrain::Plain; 50];
        let terrain = RoomEdgeTerrain::new_from_terrain_slices(&edge, &wall_edge, &edge, &edge).unwrap();
        let exits_data = RoomExitsData::new_from_compressed_edge_terrain_data(terrain, room_name);

        assert_eq!(1, exits_data.num_top_exits(), "Top exit count mismatch");
        assert_eq!(0, exits_data.num_right_exits(), "Right exit count mismatch");
        assert_eq!(1, exits_data.num_bottom_exits(), "Bottom exit count mismatch");
        assert_eq!(1, exits_data.num_left_exits(), "Left exit count mismatch");

        // -- First exit should be the top edge exit
        let exit_opt = exits_data.get_exit_by_index(0);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Top, "Exit direction invalid");

        // -- Second exit should be the bottom edge exit
        let exit_opt = exits_data.get_exit_by_index(1);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Bottom, "Exit direction invalid");

        // -- Third exit should be the left edge exit
        let exit_opt = exits_data.get_exit_by_index(2);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Left, "Exit direction invalid");
    }

    // - No bottom exits
    #[test]
    pub fn room_exits_data_get_exit_by_index_returns_some_for_exits_except_bottom_edge() {
        let room_name = RoomName::new("W0N0").unwrap();

        let wall_edge = [Terrain::Wall; 50];
        let edge = [Terrain::Plain; 50];
        let terrain = RoomEdgeTerrain::new_from_terrain_slices(&edge, &edge, &wall_edge, &edge).unwrap();
        let exits_data = RoomExitsData::new_from_compressed_edge_terrain_data(terrain, room_name);

        assert_eq!(1, exits_data.num_top_exits(), "Top exit count mismatch");
        assert_eq!(1, exits_data.num_right_exits(), "Right exit count mismatch");
        assert_eq!(0, exits_data.num_bottom_exits(), "Bottom exit count mismatch");
        assert_eq!(1, exits_data.num_left_exits(), "Left exit count mismatch");

        // -- First exit should be the top edge exit
        let exit_opt = exits_data.get_exit_by_index(0);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Top, "Exit direction invalid");

        // -- Second exit should be the right edge exit
        let exit_opt = exits_data.get_exit_by_index(1);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Right, "Exit direction invalid");

        // -- Third exit should be the left edge exit
        let exit_opt = exits_data.get_exit_by_index(2);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Left, "Exit direction invalid");
    }

    // - No left exits
    #[test]
    pub fn room_exits_data_get_exit_by_index_returns_some_for_exits_except_left_edge() {
        let room_name = RoomName::new("W0N0").unwrap();

        let wall_edge = [Terrain::Wall; 50];
        let edge = [Terrain::Plain; 50];
        let terrain = RoomEdgeTerrain::new_from_terrain_slices(&edge, &edge, &edge, &wall_edge).unwrap();
        let exits_data = RoomExitsData::new_from_compressed_edge_terrain_data(terrain, room_name);

        assert_eq!(1, exits_data.num_top_exits(), "Top exit count mismatch");
        assert_eq!(1, exits_data.num_right_exits(), "Right exit count mismatch");
        assert_eq!(1, exits_data.num_bottom_exits(), "Bottom exit count mismatch");
        assert_eq!(0, exits_data.num_left_exits(), "Left exit count mismatch");

        // -- First exit should be the top edge exit
        let exit_opt = exits_data.get_exit_by_index(0);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Top, "Exit direction invalid");

        // -- Second exit should be the right edge exit
        let exit_opt = exits_data.get_exit_by_index(1);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Right, "Exit direction invalid");

        // -- Third exit should be the bottom edge exit
        let exit_opt = exits_data.get_exit_by_index(2);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Bottom, "Exit direction invalid");
    }

    // - No top or right exits
    #[test]
    pub fn room_exits_data_get_exit_by_index_returns_some_for_exits_except_top_or_right() {
        let room_name = RoomName::new("W0N0").unwrap();

        let wall_edge = [Terrain::Wall; 50];
        let edge = [Terrain::Plain; 50];
        let terrain = RoomEdgeTerrain::new_from_terrain_slices(&wall_edge, &wall_edge, &edge, &edge).unwrap();
        let exits_data = RoomExitsData::new_from_compressed_edge_terrain_data(terrain, room_name);

        assert_eq!(0, exits_data.num_top_exits(), "Top exit count mismatch");
        assert_eq!(0, exits_data.num_right_exits(), "Right exit count mismatch");
        assert_eq!(1, exits_data.num_bottom_exits(), "Bottom exit count mismatch");
        assert_eq!(1, exits_data.num_left_exits(), "Left exit count mismatch");

        // -- Bottom edge
        let exit_opt = exits_data.get_exit_by_index(0);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Bottom, "Exit direction invalid");

        // -- Left edge
        let exit_opt = exits_data.get_exit_by_index(1);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Left, "Exit direction invalid");
    }

    // - No top, right, or bottom exits
    #[test]
    pub fn room_exits_data_get_exit_by_index_returns_some_for_exits_except_top_or_right_or_bottom() {
        let room_name = RoomName::new("W0N0").unwrap();

        let wall_edge = [Terrain::Wall; 50];
        let edge = [Terrain::Plain; 50];
        let terrain = RoomEdgeTerrain::new_from_terrain_slices(&wall_edge, &wall_edge, &wall_edge, &edge).unwrap();
        let exits_data = RoomExitsData::new_from_compressed_edge_terrain_data(terrain, room_name);

        assert_eq!(0, exits_data.num_top_exits(), "Top exit count mismatch");
        assert_eq!(0, exits_data.num_right_exits(), "Right exit count mismatch");
        assert_eq!(0, exits_data.num_bottom_exits(), "Bottom exit count mismatch");
        assert_eq!(1, exits_data.num_left_exits(), "Left exit count mismatch");

        // -- Left edge
        let exit_opt = exits_data.get_exit_by_index(0);
        assert!(exit_opt.is_some(), "Exit should exist");

        let exit = exit_opt.unwrap();

        assert_eq!(exit.start(), 1, "Exit start position invalid");
        assert_eq!(exit.end(), 48, "Exit end position invalid");
        assert_eq!(exit.len(), 48, "Exit length invalid");
        assert_eq!(exit.exit_direction(), ExitDirection::Left, "Exit direction invalid");
    }
}
