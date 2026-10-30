<?php

final class Mapper
{
    public function map($node): void
    {
        $node->value = 'mapped';

        $this->map($node);
    }
}
