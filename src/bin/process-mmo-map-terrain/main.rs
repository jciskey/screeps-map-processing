use std::env;
use std::collections::HashMap;
use screeps::{RoomName, LocalRoomTerrain};
use screeps_utils::offline_map::load_shard_map_json;

use screeps_map_processing::compressed_terrain::compressed_terrain::CompressedRoomTerrain;
use screeps_map_processing::compressed_terrain_db;


pub fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);
    let path_to_shard_map_file = &args[1];
    let output_file = &args[2];
    let terrains_map = load_all_room_terrains_from_map(&path_to_shard_map_file);

    if let Ok(conn) = compressed_terrain_db::open_db_file(output_file) {
        let create_table_res = compressed_terrain_db::create_terrain_table_if_not_exists(&conn);
        if create_table_res.is_ok() {
            for (name, terrain) in terrains_map {
                let compressed_terrain = process_terrain(&terrain);
                let insert_res = compressed_terrain_db::add_terrain_for_room(&conn, name, &compressed_terrain);
                if let Err(error) = insert_res {
                    println!("Error inserting {name}: {error}");
                }

                //break; // Only do one file for testing
            }
        }
    }
}

pub fn load_all_room_terrains_from_map(map_path: &str) -> HashMap<RoomName, LocalRoomTerrain> {
    let mut ret_data = HashMap::new();

    // Load map data
    let map_data = load_shard_map_json(map_path);

    // Extract terrain data from each room
    for (room_name, room_data) in map_data.rooms {
        ret_data.insert(room_name, room_data.terrain);
    }

    ret_data
}

pub fn process_terrain(terrain: &LocalRoomTerrain) -> CompressedRoomTerrain {
    CompressedRoomTerrain::new_from_uncompressed_bits(terrain.get_bits())
}


