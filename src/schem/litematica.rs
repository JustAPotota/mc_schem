use std::cmp::max;
use std::collections::HashMap;
use std::convert::From;
use std::fmt::format;
use std::fs::File;
use std::ops::Index;
use fastnbt::{LongArray, Value};
use flate2::Compression;
use flate2::read::{GzDecoder, GzEncoder};
use math::round::{ceil, floor};
use crate::schem::{LitematicaMetaData, Schematic, id_of_nbt_tag, RawMetaData, MetaDataIR, Region, VanillaStructureLoadOption, LitematicaLoadOption, Entity, BlockEntity, LitematicaSaveOption};
use crate::error::{LoadError, WriteError};
use crate::{schem, unwrap_opt_tag, unwrap_tag};
use crate::block::Block;

impl MetaDataIR {
    pub fn from_litematica(src: &LitematicaMetaData) -> MetaDataIR {
        return MetaDataIR {
            mc_data_version: src.data_version,
            time_created: src.time_created,
            time_modified: src.time_modified,
            author: src.author.clone(),
            name: src.name.clone(),
            description: src.description.clone(),
        }
    }
}


impl Schematic {
    pub fn from_litematica_file(filename: &str, option: &LitematicaLoadOption) -> Result<Schematic, LoadError> {
        let mut file_res = File::open(filename);
        let mut file;
        match file_res {
            Ok(f) => file = f,
            Err(e) => return Err(LoadError::FileOpenError(e)),
        }

        let mut decoder = GzDecoder::new(&mut file);
        return Self::from_litematica(&mut decoder, option);
    }
    pub fn from_litematica(src: &mut dyn std::io::Read, _option: &LitematicaLoadOption) -> Result<Schematic, LoadError> {
        let parse_res: Result<HashMap<String, Value>, fastnbt::error::Error> = fastnbt::from_reader(src);
        let parsed;
        match parse_res {
            Ok(nbt) => parsed = nbt,
            Err(e) => return Err(LoadError::NBTReadError(e)),
        }

        let mut schem = Schematic::new();
        match parse_metadata(&parsed) {
            Ok(md) => {
                schem.metadata = MetaDataIR::from_litematica(&md);
                schem.raw_metadata = Some(RawMetaData::Litematica(md));
            }
            Err(e) => return Err(e)
        }

        let regions = unwrap_opt_tag!(parsed.get("Regions"),Compound,HashMap::new(),"/Regions".to_string());
        schem.regions.reserve(regions.len());
        for (key, val) in regions {
            let reg = unwrap_tag!(val,Compound,HashMap::new(),format!("/Regions/{}",key));
            match parse_region(reg, &*format!("/Regions/{}", key)) {
                Ok(mut reg) => {
                    reg.name = key.clone();
                    schem.regions.push(reg);
                },
                Err(e) => return Err(e),
            }
        }


        return Ok(schem);
    }
}


fn parse_metadata(root: &HashMap<String, Value>) -> Result<LitematicaMetaData, LoadError> {
    let mut result = LitematicaMetaData::new();

    result.data_version = *unwrap_opt_tag!(root.get("MinecraftDataVersion"),Int,0,"/MinecraftDataVersion");
    result.version = *unwrap_opt_tag!(root.get("Version"),Int,0,"/Version");

    let md = unwrap_opt_tag!(root.get("Metadata"),Compound,HashMap::new(),"/Metadata".to_string());

    result.time_created = *unwrap_opt_tag!(md.get("TimeCreated"),Long,0,"/Metadata/TimeCreated".to_string());
    result.time_modified = *unwrap_opt_tag!(md.get("TimeModified"),Long,0,"/Metadata/TimeModified".to_string());
    {
        let enclosing_size = unwrap_opt_tag!(md.get("EnclosingSize"),Compound,HashMap::new(),"/Metadata/EnclosingSize".to_string());
        if enclosing_size.len() != 3 {
            return Err(LoadError::InvalidValue {
                tag_path: "/Metadata/EnclosingSize".to_string(),
                error: format!("Expected a compound containing 3 elements, but found {}", enclosing_size.len()),
            });
        }

        match parse_size_compound(enclosing_size, "/Metadata/EnclosingSize", false) {
            Ok(_size) => {},
            Err(e) => return Err(e),
        }

        // for dim in 0..3 {
        //     let tag_path = format!("/Metadata/EnclosingSize[{}]", dim);
        //     let val = unwrap_tag!(enclosing_size[dim],Int,0,tag_path);
        //     if val < 0 {
        //         return Err(LoadError::InvalidValue {
        //             tag_path,
        //             error: format!("Negative number {} in size", val),
        //         });
        //     }
        // }
    }

    result.description
        = unwrap_opt_tag!(md.get("Description"),String,"".to_string(),"/Metadata/Description".to_string()).clone();
    //result.total_volume = *unwrap_opt_tag!(md.get("TotalVolume"),Int,0,"/Metadata/TotalVolume".to_string()) as i64;
    result.author = unwrap_opt_tag!(md.get("Author"),String,"".to_string(),"/Metadata/Author".to_string()).clone();
    result.name = unwrap_opt_tag!(md.get("Name"),String,"".to_string(),"/Metadata/Name".to_string()).clone();

    if let Some(value) = root.get("SubVersion") {
        result.sub_version = Some(*unwrap_tag!(value,Int,0,"/SubVersion"));
    }

    return Ok(result);
}

pub fn parse_size_compound(nbt: &HashMap<String, Value>, tag_path: &str, allow_negative: bool) -> Result<[i32; 3], LoadError> {
    // let x = *unwrap_opt_tag!(nbt.get("x"),Int,0,format!("{}/x",tag_path));
    // let y = *unwrap_opt_tag!(nbt.get("y"),Int,0,format!("{}/y",tag_path));
    // let z = *unwrap_opt_tag!(nbt.get("z"),Int,0,format!("{}/z",tag_path));
    let mut result: [i32; 3] = [0, 0, 0];
    for (idx, key) in ["x", "y", "z"].iter().enumerate() {
        let cur_tag_path = format!("{}/{}", tag_path, key);
        let val = *unwrap_opt_tag!(nbt.get(*key),Int,0,cur_tag_path);
        if (!allow_negative) && (val < 0) {
            return Err(LoadError::InvalidValue {
                tag_path: cur_tag_path,
                error: format!("Expected non-negative value, but found {}", val),
            });
        }
        result[idx] = val;
    }

    return Ok(result);
}

pub fn block_required_bits(palette_size: usize) -> usize {
    let palette_size = max(palette_size, 1);
    let mut bits = 0;
    while (1 << bits) < palette_size {
        bits += 1;
    }
    return bits;
}

fn parse_region(nbt: &HashMap<String, Value>, tag_path: &str) -> Result<Region, LoadError> {
    let mut region = Region::new();

    // parse position(offset)
    {
        let cur_tag_path = format!("{}/Position", tag_path);
        let position = unwrap_opt_tag!(nbt.get("Position"),Compound,HashMap::new(),cur_tag_path);
        match parse_size_compound(position, &cur_tag_path, false) {
            Ok(pos) => region.offset = pos,
            Err(e) => return Err(e),
        }
    }

    // parse palette
    {
        let palette = unwrap_opt_tag!(nbt.get("BlockStatePalette"),List,vec![],format!("{}/BlockStatePalette",tag_path));
        region.palette.reserve(palette.len());
        for (idx, blk_nbt) in palette.iter().enumerate() {
            let cur_tag_path = format!("{}/BlockStatePalette[{}]", tag_path, idx);
            let blk_nbt = unwrap_tag!(blk_nbt,Compound,HashMap::new(),&cur_tag_path);
            let block = schem::vanilla_structure::parse_block(blk_nbt, &cur_tag_path);
            match block {
                Ok(blk) => region.palette.push(blk),
                Err(e) => return Err(e),
            }
        }
    }

    // parse size
    let region_size;
    {
        let cur_tag_path = format!("{}/Size", tag_path);
        let size = unwrap_opt_tag!(nbt.get("Size"),Compound,HashMap::new(),cur_tag_path);
        match parse_size_compound(size, &cur_tag_path, false) {
            Ok(size) => {
                region.reshape(size);
                region_size = size;
            },
            Err(e) => return Err(e),
        }
    }
    let total_blocks = region_size[0] as isize * region_size[1] as isize * region_size[2] as isize;

    //parse 3d
    {
        let palette_len = region.palette.len();
        let array =
            unwrap_opt_tag!(nbt.get("BlockStates"),LongArray,LongArray::new(vec![]),format!("{}/BlockStates",tag_path));
        let mut array_u8_be: Vec<u64> = Vec::with_capacity(array.len());
        for val in array.iter() {
            array_u8_be.push(u64::from_ne_bytes(val.to_le_bytes()));
        }
        let mbs = MultiBitSet::from_data_vec(array_u8_be, total_blocks as usize, block_required_bits(palette_len) as u8);
        assert!(mbs.is_some());
        let mbs = mbs.unwrap();
        let mut idx = 0;
        for y in 0..region.shape()[1] {
            for z in 0..region.shape()[2] {
                for x in 0..region.shape()[0] {
                    let blk_id = mbs.get(idx);
                    if blk_id >= palette_len as u64 {
                        return Err(LoadError::BlockIndexOutOfRange {
                            tag_path: format!("{}/BlockStates", tag_path),
                            index: blk_id as i32,
                            range: [0, palette_len as i32],
                        })
                    }
                    idx += 1;
                    region.array[[x as usize, y as usize, z as usize]] = blk_id as u16;
                }
            }
        }
    }

    //parse entities
    {
        let cur_tag_path = format!("{}/Entities", tag_path);
        let entities_list = unwrap_opt_tag!(nbt.get("Entities"),List,vec![],cur_tag_path);
        for (idx, entity_comp) in entities_list.iter().enumerate() {
            let cur_tag_path = format!("{}/[{}]", cur_tag_path, idx);
            let entity_comp =
                unwrap_tag!(entity_comp,Compound,HashMap::new(),cur_tag_path);
            let parse_res = parse_entity(entity_comp, &cur_tag_path);
            match parse_res {
                Ok(entity) => region.entities.push(entity),
                Err(e) => return Err(e),
            }
        }
    }

    //parse tile entities
    {
        let cur_tag_path = format!("{}/TileEntities", tag_path);
        let te_list = unwrap_opt_tag!(nbt.get("TileEntities"),List,vec![],cur_tag_path);
        for (idx, te_comp) in te_list.iter().enumerate() {
            let cur_tag_path = format!("{}/[{}]", tag_path, idx);
            let te_comp = unwrap_tag!(te_comp,Compound,HashMap::new(),cur_tag_path);

            let te_res = parse_tile_entity(te_comp, tag_path, &region_size);

            let pos;
            let te;
            match te_res {
                Ok((pos_, te_)) => {
                    pos = pos_;
                    te = te_;
                }
                Err(e) => return Err(e),
            }

            if region.block_entities.contains_key(&pos) {
                return Err(LoadError::MultipleBlockEntityInOnePos {
                    pos,
                    latter_tag_path: cur_tag_path,
                });
            }
            region.block_entities.insert(pos, te);
        }
    }

    return Ok(region);
}

#[derive(Debug)]
pub struct MultiBitSet {
    arr: Vec<u64>,
    length: usize,
    element_bits: u8,

}

pub fn ceil_up_to(a: isize, b: isize) -> isize {
    assert!(b > 0);
    if (a % b) == 0 {
        return a;
    }
    return ((a / b) + 1) * b;
}

impl MultiBitSet {
    pub fn new() -> MultiBitSet {
        return MultiBitSet {
            arr: Vec::new(),
            length: 0,
            element_bits: 1,
        }
    }

    pub fn from_data(data: &[u64], length: usize, ele_bits: u8) -> Option<MultiBitSet> {
        if ele_bits <= 0 || ele_bits > 64 {
            return None;
        }

        if (length * ele_bits as usize) > (data.len() * 64) {
            return None;
        }

        let result = MultiBitSet {
            arr: Vec::from(data),
            length,
            element_bits: ele_bits,
        };
        return Some(result);
    }

    pub fn from_data_vec(data: Vec<u64>, length: usize, ele_bits: u8) -> Option<MultiBitSet> {
        if ele_bits <= 0 || ele_bits > 64 {
            return None;
        }

        if (length * ele_bits as usize) > (data.len() * 64) {
            return None;
        }
        return Some(MultiBitSet {
            arr: data,
            length,
            element_bits: ele_bits,
        })
    }

    pub fn as_u64_slice(&self) -> &[u64] {
        return &self.arr;
    }

    pub fn element_bits(&self) -> u8 {
        return self.element_bits;
    }
    pub fn len(&self) -> usize {
        return self.length;
    }
    pub fn total_bits(&self) -> usize {
        return self.length * (self.element_bits as usize);
    }
    fn required_u64_num(&self) -> usize {
        let total_bits = self.total_bits();
        if total_bits % 64 == 0 {
            return total_bits / 64;
        }
        return total_bits / 64 + 1;
    }
    pub fn reset(&mut self, element_bits: u8, len: usize) {
        assert!(element_bits > 0);
        assert!(element_bits <= 64);
        self.length = len;
        self.element_bits = element_bits;
        self.arr.resize(self.required_u64_num(), 0);
    }


    fn global_bit_index_to_u64_index(&self, gbit_index: usize) -> usize {
        return gbit_index / 64;
    }
    fn global_bit_index_to_local_bit_index(&self, gbit_index: usize) -> usize {
        return gbit_index % 64;
    }

    fn mask_by_bits(bits: u8) -> u64 {
        if bits <= 63 {
            return (1 << (bits)) - 1;
        }
        return 0xFFFFFFFFFFFFFFFF;
    }
    fn mask_on_top_by_bits(bits: u8) -> u64 {
        assert!(bits <= 64);
        let shift_bits = 64 - bits;
        return Self::mask_by_bits(bits) << shift_bits;
    }
    pub fn basic_mask(&self) -> u64 {
        return Self::mask_by_bits(self.element_bits());
    }

    pub fn logic_bit_index_to_global_bit_index(logic_bit_index: isize) -> usize {
        assert!(logic_bit_index < 64);
        if logic_bit_index >= 0 {
            return logic_bit_index as usize;
        }
        let addon = ceil_up_to(-logic_bit_index, 64) * 2;
        //println!("logic_bit_index = {}, addon = {}", logic_bit_index, addon);
        return (logic_bit_index + addon) as usize;
    }

    fn first_global_bit_index_of(&self, ele_index: usize) -> usize {
        let logic_bit_index = 63 - ((ele_index + 1) * (self.element_bits as usize) - 1) as isize;
        return Self::logic_bit_index_to_global_bit_index(logic_bit_index);
    }
    fn last_global_bit_index_of(&self, ele_index: usize) -> usize {
        let logic_bit_index = 63 - (ele_index * (self.element_bits() as usize)) as isize;
        return Self::logic_bit_index_to_global_bit_index(logic_bit_index);
    }


    fn is_element_on_single_block(&self, ele_index: usize) -> bool {
        let fgbi = self.first_global_bit_index_of(ele_index);
        let lgbi = self.last_global_bit_index_of(ele_index);
        //assert_ne!(fgbi, lgbi);
        if fgbi > lgbi {
            return false;
        }
        return true;
    }

    pub fn element_max_value(&self) -> u64 {
        return self.basic_mask();
    }

    pub fn get(&self, ele_index: usize) -> u64 {
        assert!(ele_index < self.length);

        let fgbi = self.first_global_bit_index_of(ele_index);//first global bit index
        let lgbi = self.last_global_bit_index_of(ele_index);//last global bit index

        return if self.is_element_on_single_block(ele_index) {
            let u64_idx = self.global_bit_index_to_u64_index(fgbi);
            assert_eq!(u64_idx, self.global_bit_index_to_u64_index(lgbi));
            let llbi = self.global_bit_index_to_local_bit_index(lgbi);//last local bit index
            assert!(llbi < 64);
            let shifts = 63 - (llbi as isize);
            assert!(shifts >= 0);
            assert!(shifts + self.element_bits as isize <= 64);
            let mask = self.basic_mask() << shifts;

            let result = (self.arr[u64_idx] & mask) >> shifts;

            result
        } else {
            let u64idx_f = self.global_bit_index_to_u64_index(fgbi);
            let u64idx_l = self.global_bit_index_to_u64_index(lgbi);
            assert_eq!(u64idx_f, u64idx_l + 1);

            let l_part_bits = lgbi - u64idx_l * 64 + 1;
            let f_part_bits = ((u64idx_f + 1) * 64) - fgbi;
            assert!(l_part_bits > 0);
            assert!(f_part_bits > 0);
            assert_eq!(l_part_bits + f_part_bits, self.element_bits as usize);
            let l_mask = Self::mask_on_top_by_bits(l_part_bits as u8);
            let f_mask = Self::mask_by_bits(f_part_bits as u8);

            let l_part = (self.arr[u64idx_l] & l_mask) >> (64 - l_part_bits);
            let f_part = (self.arr[u64idx_f] & f_mask) << l_part_bits;

            let result = l_part | f_part;

            result
        }
    }

    pub fn set(&mut self, ele_index: usize, value: u64) -> Result<(), ()> {
        if value > self.element_max_value() {
            return Err(());
        }
        if ele_index >= self.length {
            return Err(());
        }
        let value_mask = self.basic_mask();
        let value = value & value_mask;

        let fgbi = self.first_global_bit_index_of(ele_index);//first global bit index
        let lgbi = self.last_global_bit_index_of(ele_index);//last global bit index
        if self.is_element_on_single_block(ele_index) {
            let u64_idx = self.global_bit_index_to_u64_index(fgbi);
            assert_eq!(u64_idx, self.global_bit_index_to_u64_index(lgbi));
            let llbi = self.global_bit_index_to_local_bit_index(lgbi);//last local bit index
            assert!(llbi < 64);
            let shifts = 63 - (llbi as isize);
            assert!(shifts >= 0);
            assert!(shifts + self.element_bits as isize <= 64);
            let mask = self.basic_mask() << shifts;

            let inv_mask = !mask;
            self.arr[u64_idx] &= inv_mask;


            self.arr[u64_idx] ^= value << shifts;
        } else {
            let u64idx_f = self.global_bit_index_to_u64_index(fgbi);
            let u64idx_l = self.global_bit_index_to_u64_index(lgbi);
            assert_eq!(u64idx_f, u64idx_l + 1);

            let l_part_bits = lgbi - u64idx_l * 64 + 1;
            let f_part_bits = ((u64idx_f + 1) * 64) - fgbi;
            assert!(l_part_bits > 0);
            assert!(f_part_bits > 0);
            assert_eq!(l_part_bits + f_part_bits, self.element_bits as usize);
            let l_mask = Self::mask_on_top_by_bits(l_part_bits as u8);
            let f_mask = Self::mask_by_bits(f_part_bits as u8);

            // erase original value
            self.arr[u64idx_f] &= !f_mask;
            self.arr[u64idx_l] &= !l_mask;

            //write new value
            let f_write_mask = (value) >> l_part_bits;
            let l_write_mask = (value) << (64 - l_part_bits);
            self.arr[u64idx_f] ^= f_write_mask;
            self.arr[u64idx_l] ^= l_write_mask;
        }


        return Ok(());
    }
}

fn parse_entity(nbt: &HashMap<String, Value>, tag_path: &str) -> Result<(Entity), LoadError> {
    let mut entity = Entity::new();
    entity.tags = nbt.clone();

    let tag_pos_path = format!("{}/Pos", tag_path);
    let pos = unwrap_opt_tag!(nbt.get("Pos"),List,vec![],tag_pos_path);
    if pos.len() != 3 {
        return Err(LoadError::InvalidValue {
            tag_path: tag_pos_path,
            error: format!("Pos filed for an entity should contain 3 doubles, but found {}", pos.len()),
        });
    }

    let mut pos_d = [0.0, 0.0, 0.0];
    for dim in 0..3 {
        let cur_tag_path = format!("{}/Pos[{}]", tag_path, dim);
        pos_d[dim] = unwrap_tag!(pos[dim],Double,0.0,cur_tag_path);
        entity.block_pos[dim] = pos_d[dim] as i32;
    }

    entity.position = pos_d;

    return Ok(entity);
}

fn parse_tile_entity(nbt: &HashMap<String, Value>, tag_path: &str, region_size: &[i32; 3])
                     -> Result<([i32; 3], BlockEntity), LoadError> {
    let mut be = BlockEntity::new();

    let pos: [i32; 3];
    let pos_res = parse_size_compound(nbt, tag_path, false);
    match pos_res {
        Ok(pos_) => pos = pos_,
        Err(e) => return Err(e),
    }

    let tag_names = ['x', 'y', 'z'];
    for (dim, p) in pos.iter().enumerate() {
        if *p < 0 || *p > region_size[dim] {
            return Err(LoadError::BlockPosOutOfRange {
                tag_path: format!("{}/{}", tag_path, tag_names[dim]),
                pos,
                range: *region_size,
            });
        }
    }

    for (key, val) in nbt {
        if key == "x" || key == "y" || key == "z" {
            continue;
        }
        be.tags.insert(key.clone(), val.clone());
    }

    return Ok((pos, be));
}


impl Schematic {
    pub fn metadata_litematica(&self) -> LitematicaMetaData {
        let mut md = LitematicaMetaData::new();
        if let Some(raw) = &self.raw_metadata {
            if let RawMetaData::Litematica(raw) = &raw {
                md = raw.clone();
            }
        }

        md.data_version = self.metadata.mc_data_version;
        md.author = self.metadata.author.clone();
        md.name = self.metadata.name.clone();
        md.description = self.metadata.description.clone();


        return md;
    }

    fn find_non_duplicate_name<T>(saved_regions: &HashMap<String, T>, old_name: &str) -> String {
        let idx = 1u64;
        loop {
            let cur_name = format!("{}({})", old_name, idx);
            if saved_regions.contains_key(&cur_name) {
                continue;
            }
            return cur_name;
        }
    }
    pub fn to_nbt_litematica(&self, option: &LitematicaSaveOption) -> Result<HashMap<String, Value>, WriteError> {
        let mut nbt: HashMap<String, Value> = HashMap::new();

        //Regions
        {
            let mut regions: HashMap<String, Value> = HashMap::with_capacity(self.regions.len());
            for reg in &self.regions {
                let nbt_region;
                match region_to_nbt_litematica(&reg) {
                    Ok(nbt) => nbt_region = nbt,
                    Err(e) => return Err(e),
                }

                if regions.contains_key(&reg.name) {
                    if option.rename_duplicated_regions {
                        let new_name = Self::find_non_duplicate_name(&regions, &reg.name);
                        regions.insert(new_name, Value::Compound(nbt_region));
                        continue;
                    }
                    return Err(WriteError::DuplicatedRegionName { name: reg.name.clone() });
                }
                regions.insert(reg.name.clone(), Value::Compound(nbt_region));
            }
            nbt.insert("Regions".to_string(), Value::Compound(regions));
        }

        // meta data
        {
            let md = self.metadata_litematica();
            nbt.insert("MinecraftDataVersion".to_string(), Value::Int(md.data_version));
            nbt.insert("Version".to_string(), Value::Int(md.version));
            if let Some(sv) = md.sub_version {
                nbt.insert("SubVersion".to_string(), Value::Int(sv));
            }
            {
                let mut md_nbt = HashMap::new();
                md_nbt.insert("Name".to_string(), Value::String(md.name));
                md_nbt.insert("Author".to_string(), Value::String(md.author));
                md_nbt.insert("Description".to_string(), Value::String(md.description));
                md_nbt.insert("TimeCreated".to_string(), Value::Long(md.time_created));
                md_nbt.insert("TimeModified".to_string(), Value::Long(md.time_modified));
                md_nbt.insert("TotalVolume".to_string(), Value::Int(self.volume() as i32));
                md_nbt.insert("TotalBlocks".to_string(), Value::Int(self.total_blocks(false) as i32));
                md_nbt.insert("RegionCount".to_string(), Value::Int(self.regions.len() as i32));
                md_nbt.insert("EnclosingSize".to_string(), Value::Compound(size_to_compound(&self.shape())));

                nbt.insert("Metadata".to_string(), Value::Compound(md_nbt));
            }
        }
        return Ok(nbt);
    }

    pub fn save_litematica_file(&self, filename: &str, option: &LitematicaSaveOption) -> Result<(), WriteError> {
        let nbt;
        match self.to_nbt_litematica(option) {
            Ok(nbt_) => nbt = nbt_,
            Err(e) => return Err(e),
        }

        let mut file;
        match File::create(filename) {
            Ok(f) => file = f,
            Err(e) => return Err(WriteError::FileCreateError(e)),
        }

        let encoder = GzEncoder::new(&mut file, Compression::best());

        let res: Result<(), fastnbt::error::Error> = fastnbt::to_writer(encoder, &nbt);
        if let Err(e) = res {
            return Err(WriteError::NBTWriteError(e));
        }

        return Ok(());
    }
}

fn region_to_nbt_litematica(region: &Region) -> Result<HashMap<String, Value>, WriteError> {
    let mut nbt = HashMap::new();
    //Size
    nbt.insert("Size".to_string(), Value::Compound(size_to_compound(&region.shape())));
    //Position
    nbt.insert("Position".to_string(), Value::Compound(size_to_compound(&region.offset)));
    // BlockStatePalette
    {
        let mut palette_vec = Vec::with_capacity(region.palette.len());
        for blk in &region.palette {
            palette_vec.push(Value::Compound(blk.to_nbt()));
        }
        nbt.insert("BlockStatePalette".to_string(), Value::List(palette_vec));
    }
    //Entities
    {
        let mut entities = Vec::with_capacity(region.entities.len());
        for entity in &region.entities {
            let mut e_nbt = entity.tags.clone();
            e_nbt.insert("Pos".to_string(), Value::List(size_to_list(&entity.position)));
            entities.push(Value::Compound(e_nbt));
        }
        nbt.insert("Entities".to_string(), Value::List(entities));
    }
    // BlockStates
    {
        let mut mbs = MultiBitSet::new();
        mbs.reset(block_required_bits(region.palette.len()) as u8, region.volume() as usize);
        let mut idx = 0usize;
        for y in 0..region.shape()[1] as usize {
            for z in 0..region.shape()[2] as usize {
                for x in 0..region.shape()[0] as usize {
                    let res = mbs.set(idx, region.array[[x, y, z]] as u64);
                    assert!(res.is_ok());
                    idx += 1;
                }
            }
        }

        let u64_slice = mbs.as_u64_slice();
        let mut i64_rep = Vec::with_capacity(u64_slice.len());
        for u_val in u64_slice {
            i64_rep.push(i64::from_le_bytes(u_val.to_ne_bytes()));
        }
        nbt.insert("BlockStates".to_string(), Value::LongArray(LongArray::new(i64_rep)));
    }
    //TileEntities
    {
        let mut te_list = Vec::with_capacity(region.block_entities.len());
        for (pos, te) in &region.block_entities {
            let mut nbt = te.tags.clone();
            nbt.insert("x".to_string(), Value::Int(pos[0]));
            nbt.insert("y".to_string(), Value::Int(pos[1]));
            nbt.insert("z".to_string(), Value::Int(pos[2]));
            te_list.push(Value::Compound(nbt));
        }
        nbt.insert("TileEntities".to_string(), Value::List(te_list));
    }
    //PendingFluidTicks
    nbt.insert("PendingFluidTicks".to_string(), Value::List(vec![]));
    //PendingBlockTicks
    nbt.insert("PendingBlockTicks".to_string(), Value::List(vec![]));

    return Ok(nbt);
}

fn size_to_compound<T>(size: &[T; 3]) -> HashMap<String, Value>
    where T: Copy, Value: From<T>
{
    return HashMap::from([("x".to_string(), Value::from(size[0])),
        ("y".to_string(), Value::from(size[1])),
        ("z".to_string(), Value::from(size[2]))]);
}

fn size_to_list<T>(size: &[T; 3]) -> Vec<Value>
    where T: Copy, Value: From<T> {
    return vec![Value::from(size[0]), Value::from(size[1]), Value::from(size[2])];
}