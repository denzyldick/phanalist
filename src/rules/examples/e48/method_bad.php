<?php

final class Walker
{
    public function walk($node): void
    {
        $this->walk($node);
    }
}
