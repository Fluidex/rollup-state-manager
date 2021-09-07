use super::order;
use crate::account::Signature;
use crate::types::merkle_tree::MerklePath;
use anyhow::bail;
use anyhow::Result;
use fluidex_common::ff::Field;
use fluidex_common::types::{Float864, FrExt};
use fluidex_common::Fr;
use std::convert::TryInto;

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum TxType {
    Nop,
    Deposit,
    Transfer,
    Withdraw,
    PlaceOrder,
    SpotTrade,
}

pub struct RawTx {
    pub tx_type: TxType,
    pub payload: Vec<Fr>,
    pub balance_path0: MerklePath,
    pub balance_path1: MerklePath,
    pub balance_path2: MerklePath,
    pub balance_path3: MerklePath,
    pub order_path0: MerklePath,
    pub order_path1: MerklePath,
    pub order_root0: Fr,
    pub order_root1: Fr,
    pub account_path0: MerklePath,
    pub account_path1: MerklePath,
    pub root_before: Fr,
    pub root_after: Fr,
    pub offset: Option<i64>,
    // debug info
    // extra: any;
}
pub type AmountType = Float864;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct L2Key {
    pub eth_addr: Fr,
    pub sign: Fr,
    pub ay: Fr,
}

pub enum L2Tx {
    Deposit(DepositTx),
    Transfer(TransferTx),
    FullSpotTrade(FullSpotTradeTx),
    Withdraw(WithdrawTx),
}

#[derive(Debug)]
pub struct DepositTx {
    pub account_id: u32,
    pub token_id: u32,
    pub amount: AmountType,
    // when l2key is none, deposit to existed account
    // else, deposit and create new account
    pub l2key: Option<L2Key>,
}

#[derive(Debug)]
pub struct SpotTradeTx {
    pub order1_account_id: u32,
    pub order2_account_id: u32,
    pub token_id_1to2: u32,
    pub token_id_2to1: u32,
    pub amount_1to2: AmountType,
    pub amount_2to1: AmountType,
    pub order1_id: u32,
    pub order2_id: u32,
}

#[derive(Debug)]
pub struct FullSpotTradeTx {
    pub trade: SpotTradeTx,
    // when xx_order is none, GlobalState must already has the order
    pub maker_order: Option<order::Order>,
    pub taker_order: Option<order::Order>,
}

#[derive(Debug, Clone)]
pub struct TransferTx {
    pub from: u32,
    pub to: u32,
    pub token_id: u32,
    pub amount: AmountType,
    pub from_nonce: Fr,
    pub sig: Signature,
    pub l2key: Option<L2Key>,
}

impl TransferTx {
    pub fn new(from: u32, to: u32, token_id: u32, amount: AmountType) -> Self {
        Self {
            from,
            to,
            token_id,
            amount,
            from_nonce: Fr::zero(),
            sig: Signature::default(),
            l2key: None,
        }
    }

    pub fn hash(&self) -> Fr {
        // adhoc ... FIXME
        // fluidex.js / dingir-exchange does not handle precision correctly now
        let amount = Fr::from_u32(self.amount.to_fr().to_i64() as u32 / 1000000);
        Fr::hash(&[
            Fr::from_u32(TxType::Transfer as u32),
            Fr::from_u32(self.token_id),
            amount,
            Fr::from_u32(self.from),
            self.from_nonce,
            Fr::from_u32(self.to),
        ])
        /*
        let data = Fr::hash(&[
            Fr::from_u32(TxType::Transfer as u32),
            Fr::from_u32(self.token_id),
            self.amount.to_fr(),
        ]);
        // do we really need to sign oldBalance?
        // i think we don't need to sign old_balance_from/to_nonce/old_balance_to?
        let data = Fr::hash(&[data, Fr::from_u32(self.from), self.from_nonce]);
        Fr::hash(&[data, Fr::from_u32(self.to)])
        */
    }
}

// WithdrawTx can only withdraw to one's own L1 address
#[derive(Debug)]
pub struct WithdrawTx {
    pub account_id: u32,
    pub token_id: u32,
    pub amount: AmountType,
    pub nonce: Fr,
    pub old_balance: Fr,
    pub sig: Signature,
}

impl WithdrawTx {
    pub fn new(account_id: u32, token_id: u32, amount: AmountType, _old_balance: Fr) -> Self {
        Self {
            account_id,
            token_id,
            amount,
            nonce: Fr::zero(),
            old_balance: Fr::zero(), // TODO: Use real `old_balance` for hash.
            sig: Signature::default(),
        }
    }

    pub fn hash(&self) -> Fr {
        // adhoc ... FIXME
        // fluidex.js / dingir-exchange does not handle precision correctly now
        let amount = Fr::from_u32(self.amount.to_fr().to_i64() as u32 / 1000000);
        Fr::hash(&[
            Fr::from_u32(TxType::Withdraw as u32),
            Fr::from_u32(self.account_id),
            Fr::from_u32(self.token_id),
            amount,
            self.nonce,
            self.old_balance,
        ])
    }
}

// DepositToNew 1 + 4 + 2 + 9 + 32 + 1 + 32 = 81
pub const PUBDATA_LEN: usize = 81;
pub const ACCOUNT_ID_LEN: usize = 4;
pub const TOKEN_ID_LEN: usize = 2;
pub const AMOUNT_LEN: usize = 9;
pub const FR_LEN: usize = 32;
//pub type PUBDATA = [u8; PUBDATA_LEN];

// https://github.com/fluidex/circuits/issues/144
impl DepositTx {
    pub fn to_pubdata(&self) -> Vec<u8> {
        let mut result = vec![TxType::Deposit as u8];
        result.append(&mut self.account_id.to_be_bytes().to_vec());
        result.append(&mut (self.token_id as u16).to_be_bytes().to_vec());
        result.append(&mut self.amount.encode());
        let l2key = self.l2key.clone().unwrap_or_default();
        result.append(&mut (l2key.ay.to_vec_be()));
        result.append(&mut [l2key.sign.to_bool().unwrap() as u8].to_vec());
        result.append(&mut (l2key.eth_addr.to_vec_be()));
        //println!("{}, {}", result.len(), PUBDATA_LEN);
        assert!(result.len() <= PUBDATA_LEN);
        result.append(&mut vec![0; PUBDATA_LEN - result.len()]);
        result
    }
    pub fn from_pubdata(data: &[u8]) -> Result<Self> {
        if data.len() != PUBDATA_LEN {
            bail!("invalid len for DepositTx");
        }
        let mut idx: usize = 0;

        if data[0] != TxType::Deposit as u8 {
            bail!("invalid type for DepositTx");
        }
        idx += 1;

        let account_id = u32::from_be_bytes(data[idx..(idx + ACCOUNT_ID_LEN)].try_into()?);
        idx += ACCOUNT_ID_LEN;

        let token_id = (u16::from_be_bytes(data[idx..(idx + TOKEN_ID_LEN)].try_into()?)) as u32;
        idx += TOKEN_ID_LEN;

        let amount = AmountType::decode(&data[idx..(idx + AMOUNT_LEN)])?;
        idx += AMOUNT_LEN;

        let ay = Fr::from_slice(&data[idx..(idx + FR_LEN)])?;
        idx += FR_LEN;
        if ay.is_zero() {
            return Ok(Self {
                account_id,
                token_id,
                amount,
                l2key: None,
            });
        }

        let sign = Fr::from_slice(&data[idx..(idx + 1)])?;
        idx += 1;
        if sign != Fr::one() && sign != Fr::zero() {
            bail!("invalid l2 account sign");
        }

        let eth_addr = Fr::from_slice(&data[idx..(idx + FR_LEN)])?;
        //idx += FR_LEN;

        Ok(Self {
            account_id,
            token_id,
            amount,
            l2key: Some(L2Key { eth_addr, sign, ay }),
        })
    }
}
/*
impl DepositToOldTx {
    pub fn to_pubdata(&self) -> Vec<u8> {
        let mut result = vec![TxType::DepositToOld as u8];
        result.append(&mut self.account_id.to_be_bytes().to_vec());
        result.append(&mut (self.token_id as u16).to_be_bytes().to_vec());
        result.append(&mut self.amount.encode());
        assert!(result.len() <= PUBDATA_LEN);
        result.append(&mut vec![0; PUBDATA_LEN - result.len()]);
        result
    }
    pub fn from_pubdata(data: &[u8]) -> Result<Self> {
        if data.len() != PUBDATA_LEN {
            bail!("invalid len for DepositToOldTx");
        }
        let mut idx: usize = 0;

        if data[0] != TxType::DepositToOld as u8 {
            bail!("invalid type for DepositToOldTx");
        }
        idx += 1;

        let account_id = u32::from_be_bytes(data[idx..(idx + ACCOUNT_ID_LEN)].try_into()?);
        idx += ACCOUNT_ID_LEN;

        let token_id = (u16::from_be_bytes(data[idx..(idx + TOKEN_ID_LEN)].try_into()?)) as u32;
        idx += TOKEN_ID_LEN;

        let amount = AmountType::decode(&data[idx..(idx + AMOUNT_LEN)])?;
        Ok(Self {
            account_id,
            token_id,
            amount,
        })
    }
}
*/
#[cfg(test)]
#[test]
fn test_deposit_to_old_pubdata() {
    let tx = DepositTx {
        account_id: 1323,
        token_id: 232,
        amount: AmountType {
            significand: 756,
            exponent: 11,
        },
        l2key: None,
    };
    let pubdata1 = tx.to_pubdata();
    println!("pubdata {:?}", pubdata1);
    let tx2 = DepositTx::from_pubdata(&pubdata1).unwrap();
    assert_eq!(tx.account_id, tx2.account_id);
    assert_eq!(tx.token_id, tx2.token_id);
    assert_eq!(tx.amount.to_bigint(), tx2.amount.to_bigint());
    assert!(tx2.l2key.is_none());
}

#[cfg(test)]
#[test]
fn test_deposit_to_new_pubdata() {
    let tx = DepositTx {
        account_id: 1323,
        token_id: 232,
        amount: AmountType {
            significand: 756,
            exponent: 11,
        },
        l2key: Some(L2Key {
            eth_addr: Fr::from_u64(1223232332323233),
            sign: Fr::from_u64(1),
            ay: Fr::from_u64(987657654765),
        }),
    };
    let pubdata1 = tx.to_pubdata();
    println!("pubdata {:?}", pubdata1);
    let tx2 = DepositTx::from_pubdata(&pubdata1).unwrap();
    assert_eq!(tx.account_id, tx2.account_id);
    assert_eq!(tx.token_id, tx2.token_id);
    assert_eq!(tx.amount.to_bigint(), tx2.amount.to_bigint());
    assert_eq!(tx.l2key.unwrap(), tx2.l2key.unwrap());
}
