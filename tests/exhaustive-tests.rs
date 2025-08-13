use screeps_map_processing::compressed_terrain::compressed_room_edge_terrain::RoomEdgeTerrain;
use screeps::Terrain;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

#[test]
pub fn room_edge_terrain_get_edge_terrain_from_bytes_copies_data_correctly() {
    let num_permutations_tested = 1_000_000_usize;
    let progress_bar = ProgressBar::new(num_permutations_tested.try_into().expect("value should always be well below <= u64::MAX"));
    let style = ProgressStyle::with_template("{percent}/100% [{elapsed_precise}] {bar:70} {human_pos:>7}/{human_len:7} ({per_sec}) {eta_precise}")
                    .unwrap()
                    .progress_chars("##-");
    progress_bar.set_style(style);

    std::iter::repeat_n(0, num_permutations_tested).par_bridge().for_each(|_| {
        // Generate the permutation array
        let mut bytes_arr = [0u8; 6];
        rand::fill(&mut bytes_arr[..]);

        // Construct the associated terrain from the permutation array
        let mut original_terrain: [Terrain; 48] = [Terrain::Plain; 48];
        (&bytes_arr).iter().enumerate().for_each(|(i, byte)| {
            for b_i in 0..8 {
                let bit = (byte >> (7 - b_i)) & 1;
                if bit > 0 {
                    // Safety:
                    // - b_i is always in the inclusive range [0, 7]
                    //   - It's an explicitly defined range
                    // - i is always in the inclusive range [0, 5]
                    //   - bytes_arr is an array of 6 elements, therefore the enumerate method will
                    //     always cap out at 5 for the index, since it's 0-based
                    // - The maximum value of final_index is 5 * 8 + 7 = 47
                    // - The maximum 0-based index of a 48 element array is 47, so this will never
                    //   overflow our array and cause a panic
                    let final_index = i * 8 + b_i;
                    original_terrain[final_index] = Terrain::Wall;
                }
            }
        });

        // Run the byte through the copy terrain function
        let output = RoomEdgeTerrain::get_edge_terrain_from_bytes(&bytes_arr);

        // Verify that the output matches the original bytes
        assert_eq!(output[0], Terrain::Wall);
        assert_eq!(output[49], Terrain::Wall);
        assert_eq!(output[1..=48], original_terrain);

        progress_bar.inc(1);
    });
}
