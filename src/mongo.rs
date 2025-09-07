#![cfg(feature = "mongodb")]

use mongodb::{
    bson::Document,
    options::{
        AggregateOptions, CountOptions, DeleteOptions, DistinctOptions, FindOneOptions, FindOptions,
        InsertManyOptions, InsertOneOptions, UpdateModifications, UpdateOptions,
    },
    results::{DeleteResult, InsertManyResult, InsertOneResult, UpdateResult},
    Collection, Cursor, Database,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::stats::QueryKind;
use crate::task_ctx::{mark, mark_latency};

// Optional DB wrapper; use TrackedDatabaseExt::tracked()
pub trait TrackedDatabaseExt {
    fn tracked(self) -> TrackedDatabase;
}
impl TrackedDatabaseExt for Database {
    fn tracked(self) -> TrackedDatabase { TrackedDatabase { inner: self } }
}

#[derive(Clone)]
pub struct TrackedDatabase {
    inner: Database,
}
impl TrackedDatabase {
    pub fn inner(&self) -> &Database { &self.inner }
    pub fn collection<T>(&self, name: &str) -> TrackedCollection<T> {
        TrackedCollection { inner: self.inner.collection::<T>(name), coll: name.to_string() }
    }
}

#[derive(Clone)]
pub struct TrackedCollection<T> {
    inner: Collection<T>,
    coll: String,
}

impl<T> TrackedCollection<T> {
    fn key(&self, op: &str) -> String { format!("{}/{}", self.coll, op) }

    pub async fn find_one(
        &self,
        filter: impl Into<Option<Document>>,
        options: impl Into<Option<FindOneOptions>>,
    ) -> mongodb::error::Result<Option<T>>
    where
        T: DeserializeOwned + Unpin + Send + Sync,
    {
        let key = self.key("find_one");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.find_one(filter, options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub async fn find(
        &self,
        filter: impl Into<Option<Document>>,
        options: impl Into<Option<FindOptions>>,
    ) -> mongodb::error::Result<Cursor<T>>
    where
        T: DeserializeOwned + Unpin + Send + Sync,
    {
        let key = self.key("find");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.find(filter, options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub async fn insert_one(
        &self,
        doc: T,
        options: impl Into<Option<InsertOneOptions>>,
    ) -> mongodb::error::Result<InsertOneResult>
    where
        T: Serialize,
    {
        let key = self.key("insert_one");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.insert_one(doc, options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub async fn insert_many(
        &self,
        docs: impl IntoIterator<Item = T>,
        options: impl Into<Option<InsertManyOptions>>,
    ) -> mongodb::error::Result<InsertManyResult>
    where
        T: Serialize,
    {
        let key = self.key("insert_many");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.insert_many(docs, options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub async fn update_one(
        &self,
        filter: impl Into<Document>,
        update: impl Into<UpdateModifications>,
        options: impl Into<Option<UpdateOptions>>,
    ) -> mongodb::error::Result<UpdateResult> {
        let key = self.key("update_one");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.update_one(filter.into(), update, options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub async fn update_many(
        &self,
        filter: impl Into<Document>,
        update: impl Into<UpdateModifications>,
        options: impl Into<Option<UpdateOptions>>,
    ) -> mongodb::error::Result<UpdateResult> {
        let key = self.key("update_many");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.update_many(filter.into(), update, options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub async fn delete_one(
        &self,
        filter: impl Into<Document>,
        options: impl Into<Option<DeleteOptions>>,
    ) -> mongodb::error::Result<DeleteResult> {
        let key = self.key("delete_one");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.delete_one(filter.into(), options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub async fn delete_many(
        &self,
        filter: impl Into<Document>,
        options: impl Into<Option<DeleteOptions>>,
    ) -> mongodb::error::Result<DeleteResult> {
        let key = self.key("delete_many");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.delete_many(filter.into(), options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub async fn aggregate(
        &self,
        pipeline: impl IntoIterator<Item = Document>,
        options: impl Into<Option<AggregateOptions>>,
    ) -> mongodb::error::Result<Cursor<Document>> {
        let key = self.key("aggregate");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.aggregate(pipeline, options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub async fn count_documents(
        &self,
        filter: impl Into<Option<Document>>,
        options: impl Into<Option<CountOptions>>,
    ) -> mongodb::error::Result<u64> {
        let key = self.key("count_documents");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.count_documents(filter, options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub async fn distinct(
        &self,
        field_name: &str,
        filter: impl Into<Option<Document>>,
        options: impl Into<Option<DistinctOptions>>,
    ) -> mongodb::error::Result<Vec<mongodb::bson::Bson>> {
        let key = self.key("distinct");
        mark(QueryKind::Mongo, &key);
        let start = std::time::Instant::now();
        let out = self.inner.distinct(field_name, filter, options).await;
        mark_latency(QueryKind::Mongo, &key, start.elapsed().as_millis());
        out
    }

    pub fn inner(&self) -> &Collection<T> { &self.inner }
}
