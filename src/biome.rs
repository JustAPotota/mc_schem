/*
mc_schem is a rust library to generate, load, manipulate and save minecraft schematic files.
Copyright (C) 2024  joseph

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

use strum::{Display, EnumIter, IntoEnumIterator};

/// Biome in Minecraft
#[repr(u8)]
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, EnumIter, Display)]
pub enum Biome {
    the_void = 0,
    plains = 1,
    sunflower_plains = 2,
    snowy_plains = 3,
    ice_spikes = 4,
    desert = 5,
    swamp = 6,
    mangrove_swamp = 7,
    forest = 8,
    flower_forest = 9,
    birch_forest = 10,
    dark_forest = 11,
    old_growth_birch_forest = 12,
    old_growth_pine_taiga = 13,
    old_growth_spruce_taiga = 14,
    taiga = 15,
    snowy_taiga = 16,
    savanna = 17,
    savanna_plateau = 18,
    windswept_hills = 19,
    windswept_gravelly_hills = 20,
    windswept_forest = 21,
    windswept_savanna = 22,
    jungle = 23,
    sparse_jungle = 24,
    bamboo_jungle = 25,
    badlands = 26,
    eroded_badlands = 27,
    wooded_badlands = 28,
    meadow = 29,
    cherry_grove = 30,
    grove = 31,
    snowy_slopes = 32,
    frozen_peaks = 33,
    jagged_peaks = 34,
    stony_peaks = 35,
    river = 36,
    frozen_river = 37,
    beach = 38,
    snowy_beach = 39,
    stony_shore = 40,
    warm_ocean = 41,
    lukewarm_ocean = 42,
    deep_lukewarm_ocean = 43,
    ocean = 44,
    deep_ocean = 45,
    cold_ocean = 46,
    deep_cold_ocean = 47,
    frozen_ocean = 48,
    deep_frozen_ocean = 49,
    mushroom_fields = 50,
    dripstone_caves = 51,
    lush_caves = 52,
    deep_dark = 53,
    nether_wastes = 54,
    warped_forest = 55,
    crimson_forest = 56,
    soul_sand_valley = 57,
    basalt_deltas = 58,
    the_end = 59,
    end_highlands = 60,
    end_midlands = 61,
    small_end_islands = 62,
    end_barrens = 63,
}

impl Biome {
    pub fn from_str(id: &str) -> Option<Self> {
        if id.starts_with("minecraft:") {
            return Self::from_str(&id[10..id.len()]);
        }
        for val in Self::iter().into_iter() {
            if id == val.to_string() {
                return Some(val);
            }
        }
        return None;
    }
}

impl Default for Biome {
    fn default() -> Self {
        return Self::the_void;
    }
}
