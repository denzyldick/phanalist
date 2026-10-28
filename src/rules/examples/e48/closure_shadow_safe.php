<?php

final class Walker
{
    public function walk(array $nodes): void
    {
        array_map(
            fn ($node) => $this->walk($node),
            $nodes
        );
    }
}
