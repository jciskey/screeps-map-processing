use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use screeps_map_processing::{
    compressed_terrain::compressed_terrain::CompressedRoomTerrain,
    run_length_encoding::rle_terrain::{
        RLERoomTerrain, PackedRLERoomTerrain, WildcardRLERoomTerrain
    },
};
use screeps::LocalRoomTerrain;
use screeps::RoomXY;
use screeps::ROOM_AREA;
use screeps::local::terrain_index_to_xy;

pub fn bench_comparison_get_xy(c: &mut Criterion) {
    // Generate the raw terrain data
    let raw_terrain_bits = Box::new([0; ROOM_AREA]);

    // Create a LocalRoomTerrain
    let uncompressed_terrain = LocalRoomTerrain::new_from_bits(raw_terrain_bits);

    // Create a CompressedRoomTerrain
    let compressed_terrain = CompressedRoomTerrain::new_from_uncompressed_bits(uncompressed_terrain.get_bits());

    // Create the RLE terrains
    let naive_rle_terrain = RLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);
    let packed_rle_terrain = PackedRLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);
    let wildcard_rle_terrain = WildcardRLERoomTerrain::new_from_compressed_terrain(&compressed_terrain);

    // Generate the RoomXY positions to pull from; we want a mix of low-index, mid-index, and
    // high-index compressed byte locations, as well as all 4 internal terrain bits for each of
    // those compressed bytes
    let low_base = 0;
    let mid_base = 1000;
    let high_base = ROOM_AREA - 4;

    let low_xy: [RoomXY; 4] = [0, 1, 2, 3].map(|i| terrain_index_to_xy(low_base + i));
    let mid_xy: [RoomXY; 4] = [0, 1, 2, 3].map(|i| terrain_index_to_xy(mid_base + i));
    let high_xy: [RoomXY; 4] = [0, 1, 2, 3].map(|i| terrain_index_to_xy(high_base + i));

    // Setup and run the benchmarks
    let mut group = c.benchmark_group("RoomTerrain");

    for xy in low_xy {
        group.bench_with_input(BenchmarkId::new("LocalRoomTerrain-LowIndex", xy), &xy, 
                                           |b, xy| b.iter(|| uncompressed_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("CompressedRoomTerrain-LowIndex", xy), &xy, 
                                           |b, xy| b.iter(|| compressed_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("RLERoomTerrain-LowIndex", xy), &xy, 
                                           |b, xy| b.iter(|| naive_rle_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("PackedRLETerrain-LowIndex", xy), &xy, 
                                           |b, xy| b.iter(|| packed_rle_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("WildcardRLETerrain-LowIndex", xy), &xy, 
                                           |b, xy| b.iter(|| wildcard_rle_terrain.get_xy(*xy)));
    }

    for xy in mid_xy {
        group.bench_with_input(BenchmarkId::new("LocalRoomTerrain-MidIndex", xy), &xy, 
                                           |b, xy| b.iter(|| uncompressed_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("CompressedRoomTerrain-MidIndex", xy), &xy, 
                                           |b, xy| b.iter(|| compressed_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("RLERoomTerrain-MidIndex", xy), &xy, 
                                           |b, xy| b.iter(|| naive_rle_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("PackedRLETerrain-MidIndex", xy), &xy, 
                                           |b, xy| b.iter(|| packed_rle_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("WildcardRLETerrain-MidIndex", xy), &xy, 
                                           |b, xy| b.iter(|| wildcard_rle_terrain.get_xy(*xy)));
    }

    for xy in high_xy {
        group.bench_with_input(BenchmarkId::new("LocalRoomTerrain-HighIndex", xy), &xy, 
                                           |b, xy| b.iter(|| uncompressed_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("CompressedRoomTerrain-HighIndex", xy), &xy, 
                                           |b, xy| b.iter(|| compressed_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("RLERoomTerrain-HighIndex", xy), &xy, 
                                           |b, xy| b.iter(|| naive_rle_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("PackedRLETerrain-HighIndex", xy), &xy, 
                                           |b, xy| b.iter(|| packed_rle_terrain.get_xy(*xy)));
        group.bench_with_input(BenchmarkId::new("WildcardRLETerrain-HighIndex", xy), &xy, 
                                           |b, xy| b.iter(|| wildcard_rle_terrain.get_xy(*xy)));
    }
}

criterion_group!(benches, bench_comparison_get_xy);
criterion_main!(benches);

