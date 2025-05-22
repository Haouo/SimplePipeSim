use super::replacement_policy::ReplacementPolicy;

pub struct GeneralCacheSetUnit<RP>
where
    RP: ReplacementPolicy,
{
    // metadata
    num_associativity: usize,
    bytes_per_block: usize,
    // core data
    data_array: Box<[u8]>,
    tag_array: Box<[u32]>,
    valid_array: Box<[bool]>,
    dirty_array: Box<[bool]>,
    rp: RP,
}

impl<RP> GeneralCacheSetUnit<RP>
where
    RP: ReplacementPolicy,
{
    pub fn new(num_associativity: usize, bytes_per_block: usize) -> Self {
        Self {
            num_associativity,
            bytes_per_block,
            data_array: vec![0u8; num_associativity * bytes_per_block].into_boxed_slice(),
            tag_array: vec![0u32; num_associativity].into_boxed_slice(),
            valid_array: vec![false; num_associativity].into_boxed_slice(),
            dirty_array: vec![false; num_associativity].into_boxed_slice(),
            rp: RP::new(num_associativity),
        }
    }

    pub fn tag_compare(&mut self, tag: u32) -> Result<usize, (bool, usize)> {
        for i in 0..self.num_associativity {
            if self.valid_array[i] && self.tag_array[i] == tag {
                return Ok(i); // return Ok(the number of the hit way)
            }
        }

        // if cache miss
        let (full, evict_way) = self.rp.evict();
        if full && self.dirty_array[evict_way] {
            return Err((true, evict_way));
        }
        // does not need to write-back, instead allocate directly
        // it can be two possible scenarios
        // 1. the cache set is full (i.e., all blocks in the set are valid), while the block to be evicted is clean
        // 2. the cache is not full
        Err((false, evict_way))
    }

    pub fn get_tag(&self, way_index: usize) -> u32 {
        self.tag_array[way_index]
    }

    pub fn read_block(&mut self, way_index: usize) -> Box<[u8]> {
        let mut read_data = Vec::<u8>::with_capacity(self.bytes_per_block);
        for i in 0..self.bytes_per_block {
            read_data.push(self.data_array[(self.bytes_per_block * way_index) + i]);
        }
        self.rp.promote(way_index);
        read_data.into_boxed_slice()
    }

    pub fn write_block(&mut self, way_index: usize, write_data: &[u8]) {
        // modify data block
        for (i, item) in write_data.iter().enumerate() {
            self.data_array[(self.bytes_per_block * way_index) + i] = *item;
        }
        // set dirty bit
        self.dirty_array[way_index] = true;
        // update replacement policy
        self.rp.promote(way_index);

        // *do not change tag and valid bit*
    }

    pub fn insert_block(&mut self, way_index: usize, new_tag: u32, new_data: &[u8]) {
        // modify data block
        for (i, item) in new_data.iter().enumerate() {
            self.data_array[(self.bytes_per_block * way_index) + i] = *item;
        }
        // set valid bit
        self.valid_array[way_index] = true;
        // set tag
        self.tag_array[way_index] = new_tag;
        // set dirty bit to false
        self.dirty_array[way_index] = false;
        // update replacement policy
        self.rp.insert(way_index);
    }
}
