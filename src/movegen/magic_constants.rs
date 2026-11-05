// Generated Fancy Magic constants for aqueval
use crate::types::Bitboard;

#[derive(Copy, Clone, Debug)]
pub struct MagicEntry {
    pub mask: Bitboard,
    pub magic: u64,
    pub shift: u8,
    pub offset: usize,
}

pub const TOTAL_ATTACK_ENTRIES: usize = 107648;

pub const BISHOP_MAGICS: [MagicEntry; 64] = [
    // Square 0 (offset 0)
    MagicEntry {
        mask: Bitboard(0x0040201008040200),
        magic: 0x2008021012002502,
        shift: 58,
        offset: 0,
    },
    // Square 1 (offset 64)
    MagicEntry {
        mask: Bitboard(0x0000402010080400),
        magic: 0x04D0100110628400,
        shift: 59,
        offset: 64,
    },
    // Square 2 (offset 96)
    MagicEntry {
        mask: Bitboard(0x0000004020100A00),
        magic: 0x21102080A1021010,
        shift: 59,
        offset: 96,
    },
    // Square 3 (offset 128)
    MagicEntry {
        mask: Bitboard(0x0000000040221400),
        magic: 0x2044041080000400,
        shift: 59,
        offset: 128,
    },
    // Square 4 (offset 160)
    MagicEntry {
        mask: Bitboard(0x0000000002442800),
        magic: 0x0004050402800000,
        shift: 59,
        offset: 160,
    },
    // Square 5 (offset 192)
    MagicEntry {
        mask: Bitboard(0x0000000204085000),
        magic: 0x0002010420109560,
        shift: 59,
        offset: 192,
    },
    // Square 6 (offset 224)
    MagicEntry {
        mask: Bitboard(0x0000020408102000),
        magic: 0x08040084500A0000,
        shift: 59,
        offset: 224,
    },
    // Square 7 (offset 256)
    MagicEntry {
        mask: Bitboard(0x0002040810204000),
        magic: 0x9401002104224008,
        shift: 58,
        offset: 256,
    },
    // Square 8 (offset 320)
    MagicEntry {
        mask: Bitboard(0x0020100804020000),
        magic: 0x40044350070B0100,
        shift: 59,
        offset: 320,
    },
    // Square 9 (offset 352)
    MagicEntry {
        mask: Bitboard(0x0040201008040000),
        magic: 0x90B00888088C1040,
        shift: 59,
        offset: 352,
    },
    // Square 10 (offset 384)
    MagicEntry {
        mask: Bitboard(0x00004020100A0000),
        magic: 0x0100100440444012,
        shift: 59,
        offset: 384,
    },
    // Square 11 (offset 416)
    MagicEntry {
        mask: Bitboard(0x0000004022140000),
        magic: 0x80001104008A0940,
        shift: 59,
        offset: 416,
    },
    // Square 12 (offset 448)
    MagicEntry {
        mask: Bitboard(0x0000000244280000),
        magic: 0x1042920210504048,
        shift: 59,
        offset: 448,
    },
    // Square 13 (offset 480)
    MagicEntry {
        mask: Bitboard(0x0000020408500000),
        magic: 0x0000010420048200,
        shift: 59,
        offset: 480,
    },
    // Square 14 (offset 512)
    MagicEntry {
        mask: Bitboard(0x0002040810200000),
        magic: 0x000000A410221000,
        shift: 59,
        offset: 512,
    },
    // Square 15 (offset 544)
    MagicEntry {
        mask: Bitboard(0x0004081020400000),
        magic: 0x804800829C901001,
        shift: 59,
        offset: 544,
    },
    // Square 16 (offset 576)
    MagicEntry {
        mask: Bitboard(0x0010080402000200),
        magic: 0x0040002008010120,
        shift: 59,
        offset: 576,
    },
    // Square 17 (offset 608)
    MagicEntry {
        mask: Bitboard(0x0020100804000400),
        magic: 0x8802008424280205,
        shift: 59,
        offset: 608,
    },
    // Square 18 (offset 640)
    MagicEntry {
        mask: Bitboard(0x004020100A000A00),
        magic: 0x200800010A040010,
        shift: 57,
        offset: 640,
    },
    // Square 19 (offset 768)
    MagicEntry {
        mask: Bitboard(0x0000402214001400),
        magic: 0x2420800802004008,
        shift: 57,
        offset: 768,
    },
    // Square 20 (offset 896)
    MagicEntry {
        mask: Bitboard(0x0000024428002800),
        magic: 0x0012011402A21220,
        shift: 57,
        offset: 896,
    },
    // Square 21 (offset 1024)
    MagicEntry {
        mask: Bitboard(0x0002040850005000),
        magic: 0x2002028508022208,
        shift: 57,
        offset: 1024,
    },
    // Square 22 (offset 1152)
    MagicEntry {
        mask: Bitboard(0x0004081020002000),
        magic: 0x0486200049100802,
        shift: 59,
        offset: 1152,
    },
    // Square 23 (offset 1184)
    MagicEntry {
        mask: Bitboard(0x0008102040004000),
        magic: 0x2000211101080200,
        shift: 59,
        offset: 1184,
    },
    // Square 24 (offset 1216)
    MagicEntry {
        mask: Bitboard(0x0008040200020400),
        magic: 0x8020200044140C60,
        shift: 59,
        offset: 1216,
    },
    // Square 25 (offset 1248)
    MagicEntry {
        mask: Bitboard(0x0010080400040800),
        magic: 0x0810680C05080381,
        shift: 59,
        offset: 1248,
    },
    // Square 26 (offset 1280)
    MagicEntry {
        mask: Bitboard(0x0020100A000A1000),
        magic: 0x0001442028012400,
        shift: 57,
        offset: 1280,
    },
    // Square 27 (offset 1408)
    MagicEntry {
        mask: Bitboard(0x0040221400142200),
        magic: 0x4028088008020002,
        shift: 55,
        offset: 1408,
    },
    // Square 28 (offset 1920)
    MagicEntry {
        mask: Bitboard(0x0002442800284400),
        magic: 0x25C1001041004010,
        shift: 55,
        offset: 1920,
    },
    // Square 29 (offset 2432)
    MagicEntry {
        mask: Bitboard(0x0004085000500800),
        magic: 0x0401020049080140,
        shift: 57,
        offset: 2432,
    },
    // Square 30 (offset 2560)
    MagicEntry {
        mask: Bitboard(0x0008102000201000),
        magic: 0x0004004084210400,
        shift: 59,
        offset: 2560,
    },
    // Square 31 (offset 2592)
    MagicEntry {
        mask: Bitboard(0x0010204000402000),
        magic: 0x40010900104400A0,
        shift: 59,
        offset: 2592,
    },
    // Square 32 (offset 2624)
    MagicEntry {
        mask: Bitboard(0x0004020002040800),
        magic: 0x011011480004A800,
        shift: 59,
        offset: 2624,
    },
    // Square 33 (offset 2656)
    MagicEntry {
        mask: Bitboard(0x0008040004081000),
        magic: 0x0082020200A0680B,
        shift: 59,
        offset: 2656,
    },
    // Square 34 (offset 2688)
    MagicEntry {
        mask: Bitboard(0x00100A000A102000),
        magic: 0x0800203000080082,
        shift: 57,
        offset: 2688,
    },
    // Square 35 (offset 2816)
    MagicEntry {
        mask: Bitboard(0x0022140014224000),
        magic: 0x0005020081880080,
        shift: 55,
        offset: 2816,
    },
    // Square 36 (offset 3328)
    MagicEntry {
        mask: Bitboard(0x0044280028440200),
        magic: 0x1050120080001004,
        shift: 55,
        offset: 3328,
    },
    // Square 37 (offset 3840)
    MagicEntry {
        mask: Bitboard(0x0008500050080400),
        magic: 0x0020008880030810,
        shift: 57,
        offset: 3840,
    },
    // Square 38 (offset 3968)
    MagicEntry {
        mask: Bitboard(0x0010200020100800),
        magic: 0x2241180900008C30,
        shift: 59,
        offset: 3968,
    },
    // Square 39 (offset 4000)
    MagicEntry {
        mask: Bitboard(0x0020400040201000),
        magic: 0x0201451101012400,
        shift: 59,
        offset: 4000,
    },
    // Square 40 (offset 4032)
    MagicEntry {
        mask: Bitboard(0x0002000204081000),
        magic: 0x8444016008025000,
        shift: 59,
        offset: 4032,
    },
    // Square 41 (offset 4064)
    MagicEntry {
        mask: Bitboard(0x0004000408102000),
        magic: 0x0002080104000800,
        shift: 59,
        offset: 4064,
    },
    // Square 42 (offset 4096)
    MagicEntry {
        mask: Bitboard(0x000A000A10204000),
        magic: 0x2801001490090200,
        shift: 57,
        offset: 4096,
    },
    // Square 43 (offset 4224)
    MagicEntry {
        mask: Bitboard(0x0014001422400000),
        magic: 0x0500142018001100,
        shift: 57,
        offset: 4224,
    },
    // Square 44 (offset 4352)
    MagicEntry {
        mask: Bitboard(0x0028002844020000),
        magic: 0x0300040408200400,
        shift: 57,
        offset: 4352,
    },
    // Square 45 (offset 4480)
    MagicEntry {
        mask: Bitboard(0x0050005008040200),
        magic: 0x0008008800820810,
        shift: 57,
        offset: 4480,
    },
    // Square 46 (offset 4608)
    MagicEntry {
        mask: Bitboard(0x0020002010080400),
        magic: 0x0804210204004212,
        shift: 59,
        offset: 4608,
    },
    // Square 47 (offset 4640)
    MagicEntry {
        mask: Bitboard(0x0040004020100800),
        magic: 0x000800A698800202,
        shift: 59,
        offset: 4640,
    },
    // Square 48 (offset 4672)
    MagicEntry {
        mask: Bitboard(0x0000020408102000),
        magic: 0x0411040202401000,
        shift: 59,
        offset: 4672,
    },
    // Square 49 (offset 4704)
    MagicEntry {
        mask: Bitboard(0x0000040810204000),
        magic: 0x0A008C051802000E,
        shift: 59,
        offset: 4704,
    },
    // Square 50 (offset 4736)
    MagicEntry {
        mask: Bitboard(0x00000A1020400000),
        magic: 0x1002A100A8040022,
        shift: 59,
        offset: 4736,
    },
    // Square 51 (offset 4768)
    MagicEntry {
        mask: Bitboard(0x0000142240000000),
        magic: 0x00000C0084042600,
        shift: 59,
        offset: 4768,
    },
    // Square 52 (offset 4800)
    MagicEntry {
        mask: Bitboard(0x0000284402000000),
        magic: 0x1000884048220000,
        shift: 59,
        offset: 4800,
    },
    // Square 53 (offset 4832)
    MagicEntry {
        mask: Bitboard(0x0000500804020000),
        magic: 0x0082200410208000,
        shift: 59,
        offset: 4832,
    },
    // Square 54 (offset 4864)
    MagicEntry {
        mask: Bitboard(0x0000201008040200),
        magic: 0x0222020441140022,
        shift: 59,
        offset: 4864,
    },
    // Square 55 (offset 4896)
    MagicEntry {
        mask: Bitboard(0x0000402010080400),
        magic: 0x1004080800408810,
        shift: 59,
        offset: 4896,
    },
    // Square 56 (offset 4928)
    MagicEntry {
        mask: Bitboard(0x0002040810204000),
        magic: 0x0022410801500201,
        shift: 58,
        offset: 4928,
    },
    // Square 57 (offset 4992)
    MagicEntry {
        mask: Bitboard(0x0004081020400000),
        magic: 0x010000410818020B,
        shift: 59,
        offset: 4992,
    },
    // Square 58 (offset 5024)
    MagicEntry {
        mask: Bitboard(0x000A102040000000),
        magic: 0x2044000044040410,
        shift: 59,
        offset: 5024,
    },
    // Square 59 (offset 5056)
    MagicEntry {
        mask: Bitboard(0x0014224000000000),
        magic: 0x00200C0100208801,
        shift: 59,
        offset: 5056,
    },
    // Square 60 (offset 5088)
    MagicEntry {
        mask: Bitboard(0x0028440200000000),
        magic: 0x080800200A102400,
        shift: 59,
        offset: 5088,
    },
    // Square 61 (offset 5120)
    MagicEntry {
        mask: Bitboard(0x0050080402000000),
        magic: 0x000404C010020090,
        shift: 59,
        offset: 5120,
    },
    // Square 62 (offset 5152)
    MagicEntry {
        mask: Bitboard(0x0020100804020000),
        magic: 0x1002101418808C03,
        shift: 59,
        offset: 5152,
    },
    // Square 63 (offset 5184)
    MagicEntry {
        mask: Bitboard(0x0040201008040200),
        magic: 0x0011300081040020,
        shift: 58,
        offset: 5184,
    },
];

pub const ROOK_MAGICS: [MagicEntry; 64] = [
    // Square 0 (offset 5248)
    MagicEntry {
        mask: Bitboard(0x000101010101017E),
        magic: 0xA680042040001480,
        shift: 52,
        offset: 5248,
    },
    // Square 1 (offset 9344)
    MagicEntry {
        mask: Bitboard(0x000202020202027C),
        magic: 0x40C0014010002000,
        shift: 53,
        offset: 9344,
    },
    // Square 2 (offset 11392)
    MagicEntry {
        mask: Bitboard(0x000404040404047A),
        magic: 0x0200100820804202,
        shift: 53,
        offset: 11392,
    },
    // Square 3 (offset 13440)
    MagicEntry {
        mask: Bitboard(0x0008080808080876),
        magic: 0x0900100008210004,
        shift: 53,
        offset: 13440,
    },
    // Square 4 (offset 15488)
    MagicEntry {
        mask: Bitboard(0x001010101010106E),
        magic: 0x4A00108402000820,
        shift: 53,
        offset: 15488,
    },
    // Square 5 (offset 17536)
    MagicEntry {
        mask: Bitboard(0x002020202020205E),
        magic: 0x2200040200018810,
        shift: 53,
        offset: 17536,
    },
    // Square 6 (offset 19584)
    MagicEntry {
        mask: Bitboard(0x004040404040403E),
        magic: 0x03000100220008AC,
        shift: 53,
        offset: 19584,
    },
    // Square 7 (offset 21632)
    MagicEntry {
        mask: Bitboard(0x008080808080807E),
        magic: 0x4080002044800D00,
        shift: 52,
        offset: 21632,
    },
    // Square 8 (offset 25728)
    MagicEntry {
        mask: Bitboard(0x0001010101017E00),
        magic: 0x008C800080400820,
        shift: 53,
        offset: 25728,
    },
    // Square 9 (offset 27776)
    MagicEntry {
        mask: Bitboard(0x0002020202027C00),
        magic: 0x400240012002D000,
        shift: 54,
        offset: 27776,
    },
    // Square 10 (offset 28800)
    MagicEntry {
        mask: Bitboard(0x0004040404047A00),
        magic: 0x0001001041002008,
        shift: 54,
        offset: 28800,
    },
    // Square 11 (offset 29824)
    MagicEntry {
        mask: Bitboard(0x0008080808087600),
        magic: 0x0110801000080080,
        shift: 54,
        offset: 29824,
    },
    // Square 12 (offset 30848)
    MagicEntry {
        mask: Bitboard(0x0010101010106E00),
        magic: 0x0001000500100800,
        shift: 54,
        offset: 30848,
    },
    // Square 13 (offset 31872)
    MagicEntry {
        mask: Bitboard(0x0020202020205E00),
        magic: 0x8A46000408020010,
        shift: 54,
        offset: 31872,
    },
    // Square 14 (offset 32896)
    MagicEntry {
        mask: Bitboard(0x0040404040403E00),
        magic: 0x00040010084104A2,
        shift: 54,
        offset: 32896,
    },
    // Square 15 (offset 33920)
    MagicEntry {
        mask: Bitboard(0x0080808080807E00),
        magic: 0x014A000220804401,
        shift: 53,
        offset: 33920,
    },
    // Square 16 (offset 35968)
    MagicEntry {
        mask: Bitboard(0x00010101017E0100),
        magic: 0x80102A8000400088,
        shift: 53,
        offset: 35968,
    },
    // Square 17 (offset 38016)
    MagicEntry {
        mask: Bitboard(0x00020202027C0200),
        magic: 0x0020008020804000,
        shift: 54,
        offset: 38016,
    },
    // Square 18 (offset 39040)
    MagicEntry {
        mask: Bitboard(0x00040404047A0400),
        magic: 0x4010008010200081,
        shift: 54,
        offset: 39040,
    },
    // Square 19 (offset 40064)
    MagicEntry {
        mask: Bitboard(0x0008080808760800),
        magic: 0x0208010100100020,
        shift: 54,
        offset: 40064,
    },
    // Square 20 (offset 41088)
    MagicEntry {
        mask: Bitboard(0x00101010106E1000),
        magic: 0x2091010008001005,
        shift: 54,
        offset: 41088,
    },
    // Square 21 (offset 42112)
    MagicEntry {
        mask: Bitboard(0x00202020205E2000),
        magic: 0x0002008080020400,
        shift: 54,
        offset: 42112,
    },
    // Square 22 (offset 43136)
    MagicEntry {
        mask: Bitboard(0x00404040403E4000),
        magic: 0x240024001110C208,
        shift: 54,
        offset: 43136,
    },
    // Square 23 (offset 44160)
    MagicEntry {
        mask: Bitboard(0x00808080807E8000),
        magic: 0x0400120001008054,
        shift: 53,
        offset: 44160,
    },
    // Square 24 (offset 46208)
    MagicEntry {
        mask: Bitboard(0x000101017E010100),
        magic: 0x8080208080004004,
        shift: 53,
        offset: 46208,
    },
    // Square 25 (offset 48256)
    MagicEntry {
        mask: Bitboard(0x000202027C020200),
        magic: 0x80DD5004C0042000,
        shift: 54,
        offset: 48256,
    },
    // Square 26 (offset 49280)
    MagicEntry {
        mask: Bitboard(0x000404047A040400),
        magic: 0x0410040120080120,
        shift: 54,
        offset: 49280,
    },
    // Square 27 (offset 50304)
    MagicEntry {
        mask: Bitboard(0x0008080876080800),
        magic: 0x2000D00180380080,
        shift: 54,
        offset: 50304,
    },
    // Square 28 (offset 51328)
    MagicEntry {
        mask: Bitboard(0x001010106E101000),
        magic: 0x0008000880040080,
        shift: 54,
        offset: 51328,
    },
    // Square 29 (offset 52352)
    MagicEntry {
        mask: Bitboard(0x002020205E202000),
        magic: 0x100A000200080410,
        shift: 54,
        offset: 52352,
    },
    // Square 30 (offset 53376)
    MagicEntry {
        mask: Bitboard(0x004040403E404000),
        magic: 0x0300080400100102,
        shift: 54,
        offset: 53376,
    },
    // Square 31 (offset 54400)
    MagicEntry {
        mask: Bitboard(0x008080807E808000),
        magic: 0x6200008200011044,
        shift: 53,
        offset: 54400,
    },
    // Square 32 (offset 56448)
    MagicEntry {
        mask: Bitboard(0x0001017E01010100),
        magic: 0x061481400C800060,
        shift: 53,
        offset: 56448,
    },
    // Square 33 (offset 58496)
    MagicEntry {
        mask: Bitboard(0x0002027C02020200),
        magic: 0x1001004001002084,
        shift: 54,
        offset: 58496,
    },
    // Square 34 (offset 59520)
    MagicEntry {
        mask: Bitboard(0x0004047A04040400),
        magic: 0x0000200080801000,
        shift: 54,
        offset: 59520,
    },
    // Square 35 (offset 60544)
    MagicEntry {
        mask: Bitboard(0x0008087608080800),
        magic: 0x840010010100200B,
        shift: 54,
        offset: 60544,
    },
    // Square 36 (offset 61568)
    MagicEntry {
        mask: Bitboard(0x0010106E10101000),
        magic: 0x0028040080800800,
        shift: 54,
        offset: 61568,
    },
    // Square 37 (offset 62592)
    MagicEntry {
        mask: Bitboard(0x0020205E20202000),
        magic: 0x0882000406001830,
        shift: 54,
        offset: 62592,
    },
    // Square 38 (offset 63616)
    MagicEntry {
        mask: Bitboard(0x0040403E40404000),
        magic: 0x0001005421001200,
        shift: 54,
        offset: 63616,
    },
    // Square 39 (offset 64640)
    MagicEntry {
        mask: Bitboard(0x0080807E80808000),
        magic: 0x000001804600010C,
        shift: 53,
        offset: 64640,
    },
    // Square 40 (offset 66688)
    MagicEntry {
        mask: Bitboard(0x00017E0101010100),
        magic: 0x0000804000208000,
        shift: 53,
        offset: 66688,
    },
    // Square 41 (offset 68736)
    MagicEntry {
        mask: Bitboard(0x00027C0202020200),
        magic: 0x4400402010044000,
        shift: 54,
        offset: 68736,
    },
    // Square 42 (offset 69760)
    MagicEntry {
        mask: Bitboard(0x00047A0404040400),
        magic: 0x4010008020028014,
        shift: 54,
        offset: 69760,
    },
    // Square 43 (offset 70784)
    MagicEntry {
        mask: Bitboard(0x0008760808080800),
        magic: 0x0000090410010020,
        shift: 54,
        offset: 70784,
    },
    // Square 44 (offset 71808)
    MagicEntry {
        mask: Bitboard(0x00106E1010101000),
        magic: 0x0000080100110005,
        shift: 54,
        offset: 71808,
    },
    // Square 45 (offset 72832)
    MagicEntry {
        mask: Bitboard(0x00205E2020202000),
        magic: 0x0A00201004080140,
        shift: 54,
        offset: 72832,
    },
    // Square 46 (offset 73856)
    MagicEntry {
        mask: Bitboard(0x00403E4040404000),
        magic: 0x0000040200010100,
        shift: 54,
        offset: 73856,
    },
    // Square 47 (offset 74880)
    MagicEntry {
        mask: Bitboard(0x00807E8080808000),
        magic: 0x0220007081020004,
        shift: 53,
        offset: 74880,
    },
    // Square 48 (offset 76928)
    MagicEntry {
        mask: Bitboard(0x007E010101010100),
        magic: 0x840205C981002A00,
        shift: 53,
        offset: 76928,
    },
    // Square 49 (offset 78976)
    MagicEntry {
        mask: Bitboard(0x007C020202020200),
        magic: 0x0000804000200480,
        shift: 54,
        offset: 78976,
    },
    // Square 50 (offset 80000)
    MagicEntry {
        mask: Bitboard(0x007A040404040400),
        magic: 0x0002081040802200,
        shift: 54,
        offset: 80000,
    },
    // Square 51 (offset 81024)
    MagicEntry {
        mask: Bitboard(0x0076080808080800),
        magic: 0x0240230010000900,
        shift: 54,
        offset: 81024,
    },
    // Square 52 (offset 82048)
    MagicEntry {
        mask: Bitboard(0x006E101010101000),
        magic: 0x0044800800240180,
        shift: 54,
        offset: 82048,
    },
    // Square 53 (offset 83072)
    MagicEntry {
        mask: Bitboard(0x005E202020202000),
        magic: 0x4011000400080300,
        shift: 54,
        offset: 83072,
    },
    // Square 54 (offset 84096)
    MagicEntry {
        mask: Bitboard(0x003E404040404000),
        magic: 0x00101011088A0C00,
        shift: 54,
        offset: 84096,
    },
    // Square 55 (offset 85120)
    MagicEntry {
        mask: Bitboard(0x007E808080808000),
        magic: 0x1003000080420100,
        shift: 53,
        offset: 85120,
    },
    // Square 56 (offset 87168)
    MagicEntry {
        mask: Bitboard(0x7E01010101010100),
        magic: 0x0180102100408001,
        shift: 52,
        offset: 87168,
    },
    // Square 57 (offset 91264)
    MagicEntry {
        mask: Bitboard(0x7C02020202020200),
        magic: 0x1100108040010021,
        shift: 53,
        offset: 91264,
    },
    // Square 58 (offset 93312)
    MagicEntry {
        mask: Bitboard(0x7A04040404040400),
        magic: 0x0182004008108022,
        shift: 53,
        offset: 93312,
    },
    // Square 59 (offset 95360)
    MagicEntry {
        mask: Bitboard(0x7608080808080800),
        magic: 0x0122900128202501,
        shift: 53,
        offset: 95360,
    },
    // Square 60 (offset 97408)
    MagicEntry {
        mask: Bitboard(0x6E10101010101000),
        magic: 0x0002012004100802,
        shift: 53,
        offset: 97408,
    },
    // Square 61 (offset 99456)
    MagicEntry {
        mask: Bitboard(0x5E20202020202000),
        magic: 0x00C200834C081002,
        shift: 53,
        offset: 99456,
    },
    // Square 62 (offset 101504)
    MagicEntry {
        mask: Bitboard(0x3E40404040404000),
        magic: 0x0440020110083084,
        shift: 53,
        offset: 101504,
    },
    // Square 63 (offset 103552)
    MagicEntry {
        mask: Bitboard(0x7E80808080808000),
        magic: 0x4000484884010022,
        shift: 52,
        offset: 103552,
    },
];
