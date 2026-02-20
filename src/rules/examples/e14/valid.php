<?php

// Valid: Fluent interface - methods return self/static
class QueryBuilder
{
    public function select(string $columns): self
    {
        return $this;
    }

    public function where(string $condition): self
    {
        return $this;
    }

    public function limit(int $n): static
    {
        return $this;
    }

    public function build(): string
    {
        // Chaining on $this is fine — each call returns self/static
        return $this->select('*')->where('id = 1')->limit(10)->build();
    }
}
