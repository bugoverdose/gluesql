use {
    super::{err_into, key, lock, SledStorage, Snapshot, State},
    async_trait::async_trait,
    gluesql_core::{
        data::{Key, Row, Schema},
        result::{Error, Result},
        store::{RowIter, Store},
    },
};

#[async_trait(?Send)]
impl Store for SledStorage {
    async fn fetch_schema(&self, table_name: &str) -> Result<Option<Schema>> {
        let (txid, created_at, temp) = match self.state {
            State::Transaction {
                txid, created_at, ..
            } => (txid, created_at, false),
            State::Idle => lock::register(&self.tree, self.id_offset)
                .map(|(txid, created_at)| (txid, created_at, true))?,
        };
        let lock_txid = lock::fetch(&self.tree, txid, created_at, self.tx_timeout)?;

        let key = format!("schema/{}", table_name);
        let schema = self
            .tree
            .get(key.as_bytes())
            .map_err(err_into)?
            .map(|v| bincode::deserialize(&v))
            .transpose()
            .map_err(err_into)?
            .and_then(|snapshot: Snapshot<Schema>| snapshot.extract(txid, lock_txid));

        if temp {
            lock::unregister(&self.tree, txid)?;
        }

        Ok(schema)
    }

    async fn fetch_data(&self, table_name: &str, key: &Key) -> Result<Option<Row>> {
        let (txid, created_at) = match self.state {
            State::Transaction {
                txid, created_at, ..
            } => (txid, created_at),
            State::Idle => {
                return Err(Error::StorageMsg(
                    "conflict - fetch_data failed, lock does not exist".to_owned(),
                ));
            }
        };
        let lock_txid = lock::fetch(&self.tree, txid, created_at, self.tx_timeout)?;

        let key = key::data(table_name, key.to_cmp_be_bytes());
        let row = self
            .tree
            .get(&key)
            .map_err(err_into)?
            .map(|v| bincode::deserialize(&v))
            .transpose()
            .map_err(err_into)?
            .and_then(|snapshot: Snapshot<Row>| snapshot.extract(txid, lock_txid));

        Ok(row)
    }

    async fn scan_data(&self, table_name: &str) -> Result<RowIter> {
        let (txid, created_at) = match self.state {
            State::Transaction {
                txid, created_at, ..
            } => (txid, created_at),
            State::Idle => {
                return Err(Error::StorageMsg(
                    "conflict - scan_data failed, lock does not exist".to_owned(),
                ));
            }
        };
        let lock_txid = lock::fetch(&self.tree, txid, created_at, self.tx_timeout)?;

        let prefix = key::data_prefix(table_name);
        let prefix_len = prefix.len();
        let result_set = self
            .tree
            .scan_prefix(prefix.as_bytes())
            .map(move |item| {
                let (key, value) = item.map_err(err_into)?;
                let key = key.subslice(prefix_len, key.len() - prefix_len).to_vec();
                let snapshot: Snapshot<Row> = bincode::deserialize(&value).map_err(err_into)?;
                let row = snapshot.extract(txid, lock_txid);
                let item = row.map(|row| (Key::Bytea(key), row));

                Ok(item)
            })
            .filter_map(|item| item.transpose());

        Ok(Box::new(result_set))
    }
}
