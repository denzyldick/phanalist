<?php

interface QueryBuilder {
    public function select(string $fields): QueryBuilder;
    public function where(string $condition): QueryBuilder;
}

class QueryService {
    public function buildQuery(QueryBuilder $qb) {
        $qb->select('id')->where('id = 1')->where('status = 1');
    }
}
