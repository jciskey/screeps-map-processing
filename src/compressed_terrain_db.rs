
use rusqlite::{Connection, Error};
use screeps::RoomName;
use crate::compressed_terrain::compressed_terrain::CompressedRoomTerrain;

pub fn open_db_file(path: &str) -> Result<Connection, Error> {
    Connection::open(path)
}

pub fn create_terrain_table_if_not_exists(conn: &Connection) -> Result<(), Error> {
    let table_exists = conn.table_exists(None, "room_terrain")?;

    // The existence query was successful, now actually create the table if it doesn't exist
    if !table_exists {
        // The table doesn't already exist, create it
        let _ = conn.execute_batch("CREATE TABLE room_terrain (id INTEGER PRIMARY KEY, room_name TEXT,  data BLOB);")?;
    }
    
    Ok(())
}

pub fn add_terrain_for_room(conn: &Connection, room_name: RoomName, terrain: &CompressedRoomTerrain) -> Result<(), Error> {
    let params = rusqlite::named_params!{
        ":room_name": room_name.to_string(),
        ":data": terrain.get_compressed_bytes(),
    };
    conn.execute("INSERT INTO room_terrain (room_name, data) VALUES (:room_name, :data)", params).and(Ok(()))
}

pub fn get_terrain_for_room(conn: &Connection, room_name: RoomName) -> Result<CompressedRoomTerrain, Error> {
    let params = rusqlite::named_params!{
        ":room_name": room_name.to_string(),
    };
    conn.query_row_and_then(
        "SELECT data FROM room_terrain WHERE room_name = :room_name LIMIT 1",
        params,
        |row| row.get(0).and_then(
            |bytes| Ok(CompressedRoomTerrain::new_from_compressed_bytes(Box::new(bytes)))
        )
    )
}

pub fn get_rooms_with_terrain(conn: &Connection) -> Result<Vec<RoomName>, Error> {
    let mut stmt = conn.prepare("SELECT room_name FROM room_terrain")?;
    let rows = stmt.query_map([], |row| row.get::<usize, String>(0))?;

    let mut res = Vec::new();

    for names_result in rows {
        if let Ok(name) = RoomName::new(names_result?.as_str()) {
            res.push(name);
        }
    }

    Ok(res)
}

