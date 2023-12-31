use consensus::network_config::{Network, NetworkConfig};
use frame_support::{parameter_types, sp_io, traits::GenesisBuild};
use frame_support::{
    sp_runtime::{
        testing::Header,
        traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
        AccountId32, MultiSignature,
    },
    sp_std::convert::{TryFrom, TryInto},
    PalletId,
};
use frame_system as system;
use frame_system::EnsureRoot;
use sp_core::H256;
use webb_proposals::TypedChainId;

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();
    let _ = pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (AccountId32::new([1u8; 32]), 10u128.pow(18)),
            (AccountId32::new([2u8; 32]), 20u128.pow(18)),
            (AccountId32::new([3u8; 32]), 30u128.pow(18)),
            (Eth2Client::account_id(), 40u128.pow(18)),
        ],
    }
    .assimilate_storage(&mut storage);
    let _ = pallet_eth2_light_client::GenesisConfig::<Test> {
        phantom: Default::default(),
        networks: vec![
            (
                // Mainnet
                TypedChainId::Evm(1),
                NetworkConfig::new(&Network::Mainnet),
            ),
            (
                // Goerli
                TypedChainId::Evm(5),
                NetworkConfig::new(&Network::Goerli),
            ),
        ],
    }
    .assimilate_storage(&mut storage);

    storage.into()
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
}

impl system::Config for Test {
    type AccountData = pallet_balances::AccountData<u128>;
    type AccountId = AccountId;
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockHashCount = BlockHashCount;
    type BlockLength = ();
    type BlockNumber = u64;
    type BlockWeights = ();
    type RuntimeCall = RuntimeCall;
    type DbWeight = ();
    type RuntimeEvent = RuntimeEvent;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type Header = Header;
    type Index = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type OnKilledAccount = ();
    type OnNewAccount = ();
    type OnSetCode = ();
    type RuntimeOrigin = RuntimeOrigin;
    type PalletInfo = PalletInfo;
    type SS58Prefix = SS58Prefix;
    type SystemWeightInfo = ();
    type Version = ();
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = u128;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type HoldIdentifier = ();
    type FreezeIdentifier = ();
    type MaxHolds = ();
    type MaxFreezes = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const MaxAdditionalFields: u32 = 5;
    pub const MaxResources: u32 = 32;
    pub const StoragePricePerByte: u128 = 1;
    pub const Eth2ClientPalletId: PalletId = PalletId(*b"py/eth2c");
}

impl pallet_eth2_light_client::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type StoragePricePerByte = StoragePricePerByte;
    type PalletId = Eth2ClientPalletId;
    type Currency = Balances;
}

impl pallet_receipt_registry::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = Eth2ClientPalletId;
    type Currency = Balances;
    type PrivilegedOrigin = EnsureRoot<AccountId>;
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Eth2Client: pallet_eth2_light_client::{Pallet, Call, Storage, Event<T>},
        ReceiptRegistry: pallet_receipt_registry::{Pallet, Call, Storage, Event<T>},
    }
);
