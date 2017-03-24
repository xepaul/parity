// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use std::sync::Arc;
use parking_lot::Mutex;
use ethkey::public_to_address;
use ethcore::client::{Client, BlockChainClient, BlockId};
use types::all::{Error, DocumentAddress, Public};

const ACL_CHECKER_CONTRACT_REGISTRY_NAME: &'static str = "secretstore_acl_checker";

/// ACL storage of Secret Store
pub trait AclStorage: Send + Sync {
	/// Check if requestor with `public` key can access document with hash `document`
	fn check(&self, public: &Public, document: &DocumentAddress) -> Result<bool, Error>;
}

/// On-chain ACL storage implementation.
pub struct OnChainAclStorage {
	/// Blockchain client.
	client: Arc<Client>,
	/// On-chain contract.
	contract: Mutex<Option<provider::Contract>>,
}

impl OnChainAclStorage {
	pub fn new(client: Arc<Client>) -> Self {
		OnChainAclStorage {
			client: client,
			contract: Mutex::new(None),
		}
	}
}

impl AclStorage for OnChainAclStorage {
	fn check(&self, public: &Public, document: &DocumentAddress) -> Result<bool, Error> {
		let mut contract = self.contract.lock();
		if !contract.is_some() {
			*contract = self.client.registry_address(ACL_CHECKER_CONTRACT_REGISTRY_NAME.to_owned())
				.and_then(|contract_addr| {
					trace!(target: "secretstore", "Configuring for ACL checker contract from {}", contract_addr);

					let client = Arc::downgrade(&self.client);
					Some(provider::Contract::new(contract_addr, move |a, d| client.upgrade().ok_or("No client!".into()).and_then(|c| c.call_contract(BlockId::Latest, a, d))))
				})
		}
		if let Some(ref contract) = *contract {
			let address = public_to_address(&public);
			contract.check_permissions(&address, document)
				.map_err(|err| Error::Internal(err))
		} else {
			Err(Error::Internal("ACL checker contract is not configured".to_owned()))
		}
	}
}

mod provider {
	// Autogenerated from JSON contract definition using Rust contract convertor.
	// Command line: 
	#![allow(unused_imports)]
	use std::string::String;
	use std::result::Result;
	use std::fmt;
	use {util, ethabi};
	use util::Uint;

	pub struct Contract {
		contract: ethabi::Contract,
		pub address: util::Address,
		do_call: Box<Fn(util::Address, Vec<u8>) -> Result<Vec<u8>, String> + Send + Sync + 'static>,
	}
	impl Contract {
		pub fn new<F>(address: util::Address, do_call: F) -> Self
			where F: Fn(util::Address, Vec<u8>) -> Result<Vec<u8>, String> + Send + Sync + 'static {
			Contract {
				contract: ethabi::Contract::new(ethabi::Interface::load(b"[{\"constant\":true,\"inputs\":[{\"name\":\"user\",\"type\":\"address\"},{\"name\":\"document\",\"type\":\"bytes32\"}],\"name\":\"checkPermissions\",\"outputs\":[{\"name\":\"\",\"type\":\"bool\"}],\"payable\":false,\"type\":\"function\"}]").expect("JSON is autogenerated; qed")),
				address: address,
				do_call: Box::new(do_call),
			}
		}
		fn as_string<T: fmt::Debug>(e: T) -> String { format!("{:?}", e) }
		
		/// Auto-generated from: `{"constant":true,"inputs":[{"name":"user","type":"address"},{"name":"document","type":"bytes32"}],"name":"checkPermissions","outputs":[{"name":"","type":"bool"}],"payable":false,"type":"function"}`
		#[allow(dead_code)]
		pub fn check_permissions(&self, user: &util::Address, document: &util::H256) -> Result<bool, String>
			{
			let call = self.contract.function("checkPermissions".into()).map_err(Self::as_string)?;
			let data = call.encode_call(
				vec![ethabi::Token::Address(user.clone().0), ethabi::Token::FixedBytes(document.as_ref().to_owned())]
			).map_err(Self::as_string)?;
			let output = call.decode_output((self.do_call)(self.address.clone(), data)?).map_err(Self::as_string)?;
			let mut result = output.into_iter().rev().collect::<Vec<_>>();
			Ok(({ let r = result.pop().ok_or("Invalid return arity")?; let r = r.to_bool().ok_or("Invalid type returned")?; r }))
		}
	}
}

#[cfg(test)]
pub mod tests {
	use std::collections::{HashMap, HashSet};
	use parking_lot::RwLock;
	use types::all::{Error, DocumentAddress, Public};
	use super::AclStorage;

	#[derive(Default, Debug)]
	/// Dummy ACL storage implementation
	pub struct DummyAclStorage {
		prohibited: RwLock<HashMap<Public, HashSet<DocumentAddress>>>,
	}

	impl DummyAclStorage {
		#[cfg(test)]
		/// Prohibit given requestor access to given document
		pub fn prohibit(&self, public: Public, document: DocumentAddress) {
			self.prohibited.write()
				.entry(public)
				.or_insert_with(Default::default)
				.insert(document);
		}
	}

	impl AclStorage for DummyAclStorage {
		fn check(&self, public: &Public, document: &DocumentAddress) -> Result<bool, Error> {
			Ok(self.prohibited.read()
				.get(public)
				.map(|docs| !docs.contains(document))
				.unwrap_or(true))
		}
	}
}
