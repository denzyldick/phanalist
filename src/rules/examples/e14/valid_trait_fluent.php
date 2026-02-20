<?php

// Valid: trait defines fluent methods returning self, class uses the trait and chains freely
trait FluentTrait
{
    public function withName(string $name): self
    {
        return $this;
    }

    public function withValue(int $val): self
    {
        return $this;
    }
}

class Builder
{
    use FluentTrait;

    public function build(): string
    {
        return $this->withName('foo')->withValue(42)->build();
    }
}
