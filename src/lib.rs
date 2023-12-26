//! pbc-kyc

#[macro_use]
extern crate pbc_contract_codegen;

use pbc_contract_common::address::{Address,AddressType};
use pbc_contract_common::context::{ContractContext, CallbackContext};
use pbc_contract_common::events::EventGroup;
use pbc_contract_common::shortname::Shortname;
use pbc_contract_common::sorted_vec_map::SortedVecMap;
use read_write_state_derive::ReadWriteState;
use read_write_rpc_derive::ReadWriteRPC;
use create_type_spec_derive::CreateTypeSpec;


#[state]
pub struct ContractState {
    owner: Address,
    registry_address: Address,
    storage_adddress: Address,
    kycs: SortedVecMap<u128, Kyc>, // Key: Applicant DID, Value: KYC
}

#[init]
fn initialize(
    ctx: ContractContext,
) -> ContractState {

    let kyc_storage: SortedVecMap<u128, Kyc> = SortedVecMap::new();
    let blank_address: Address = Address { address_type: AddressType::Account, identifier: [0x00; 20] };
    let state = ContractState {
        owner: ctx.sender,
        registry_address: blank_address,
        storage_adddress: blank_address,
        kycs: kyc_storage,
    };

    state
}

#[derive(ReadWriteRPC, CreateTypeSpec, ReadWriteState)]
pub struct Kyc {
    applicant_did: String,
    applicant_info: Vec<SubjectInfo>,
    approved: bool,
    pending: bool,
}

#[derive(ReadWriteRPC, CreateTypeSpec, ReadWriteState, Clone)]
pub struct SubjectInfo {
    property_name: String,
    property_value: String,
}


#[action(shortname = 0x01)]
pub fn configure_registry_address(
    context: ContractContext,
    mut state: ContractState,
    target_registry_address: Address,
    target_storage_address: Address,
) -> ContractState {

    assert!(context.sender == state.owner, "Not Authorized!");

    state.registry_address = target_registry_address;
    state.storage_adddress = target_storage_address;

    state
}

#[action(shortname = 0x02)]
pub fn upload_kyc(
    context: ContractContext,
    state: ContractState,
    applicant_did: String,
    applicant_info: Vec<SubjectInfo>,
) -> (ContractState, Vec<EventGroup>) {

    assert!(state.registry_address.identifier != [0x00; 20], "Please configure a valid DID Registry Address!");

    let mut event_group_builder = EventGroup::builder();
    let copied_did = applicant_did.clone();

    let new_kyc : Kyc = Kyc { 
        applicant_did: applicant_did,
        applicant_info: applicant_info, 
        approved: false, 
        pending: true, };
    // Call the DID Registry Contract to check if the Sender has the right to upload KVC for a certain DID
    // 0x05 is the Shortname for the method implemented on the Registry Contract, needs to be consistent
    event_group_builder
    .call(state.registry_address, Shortname::from_u32(0x05))
    .argument(copied_did)
    .argument(context.sender)
    .done();

    event_group_builder
        .with_callback(SHORTNAME_UPLOAD_KYC_CALLBACK)
        .argument(new_kyc)
        .done();

    (state, vec![event_group_builder.build()])
}


#[callback(shortname = 0x12)]
pub fn upload_kyc_callback(
    _context: ContractContext,
    callback_context: CallbackContext,
    mut state: ContractState,
    new_kyc: Kyc,
) -> (ContractState, Vec<EventGroup>) {
    assert!(callback_context.success, "DID Not Registered or Not Authorized!");

    let current_idx: u128 = state.kycs.len().try_into().unwrap();
    state.kycs.insert(current_idx, new_kyc);

    (state, vec![])
}

#[action(shortname = 0x03)]
pub fn approve_kyc(
    context: ContractContext,
    mut state: ContractState,
    kyc_idx: u128,
    decision: bool,
) -> ContractState {

    assert!(context.sender == state.owner, "Not Authorized!");
    assert!(state.kycs.contains_key(&kyc_idx), "KYC Not Found!");

    let kyc_to_approve = state.kycs.get_mut(&kyc_idx).unwrap();
    kyc_to_approve.pending = false;

    if decision {
        kyc_to_approve.approved = true;
    } else {
        kyc_to_approve.approved = false;
    }

    state
}

#[action(shortname = 0x04)]
pub fn create_vc(
    context: ContractContext,
    state: ContractState,
    kyc_idx: u128,
    issuer_did: String,
    valid_since: String,
    valid_until: String,
    description: String,
) -> (ContractState, Vec<EventGroup>) {

    assert!(context.sender == state.owner, "Not Authorized!");
    assert!(state.storage_adddress.identifier != [0x00; 20], "Please configure a valid VC Storage Address!");
    assert!(state.kycs.contains_key(&kyc_idx), "KYC Not Found!");
    assert!(state.kycs.get(&kyc_idx).unwrap().approved, "KYC Not Approved!");

    let kyc = state.kycs.get(&kyc_idx).unwrap();
    let mut event_group_builder = EventGroup::builder();
    let copied_issuer_did = issuer_did.clone();
    let copied_applicant_did = kyc.applicant_did.clone();

    // Call the VC Storage Contract to Upload a VC for the Applicant
    // 0x02 is the Shortname for the method implemented on the Registry Contract, needs to be consistent
    /* Function Signature
    #[action(shortname = 0x02)]
        pub fn upload_vc(
        context: ContractContext,
        state: ContractState,
        issuer_did: String,
        vc_id: u128,
        subject_did: String,
        subject_info: Vec<SubjectInfo>,
        valid_since: String,
        valid_until: String,
        descrption: String,
        is_revoked: bool,
    )
    */
    event_group_builder
        .call(state.storage_adddress, Shortname::from_u32(0x02))
        .argument(copied_issuer_did)
        .argument(kyc_idx)
        .argument(copied_applicant_did)
        .argument(kyc.applicant_info.clone())
        .argument(valid_since)
        .argument(valid_until)
        .argument(description)
        .argument(false)
        .done();

    event_group_builder
        .with_callback(SHORTNAME_CREATE_VC_CALLBACK)
        .done();

    (state, vec![event_group_builder.build()])
}

#[callback(shortname = 0x14)]
pub fn create_vc_callback(
    _context: ContractContext,
    callback_context: CallbackContext,
    state: ContractState,
) -> (ContractState, Vec<EventGroup>) {
    assert!(callback_context.success, "VC Failed to Upload!");

    (state, vec![])
}