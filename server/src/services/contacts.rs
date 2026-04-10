use tonic::{Request, Response, Status};
use tracing::info;

use crate::proto::contacts::*;
use crate::proto::contacts::contacts_rpc_service_server::ContactsRpcService;

pub struct ContactsRpcServiceImpl;

#[tonic::async_trait]
impl ContactsRpcService for ContactsRpcServiceImpl {
    async fn get_contacts(
        &self,
        _request: Request<GetContactsRequest>,
    ) -> Result<Response<ContactList>, Status> {
        info!(">>> Contacts.GetContacts");
        Ok(Response::new(ContactList { contacts: vec![] }))
    }
}
