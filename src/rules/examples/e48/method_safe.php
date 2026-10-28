<?php

final class Walker
{
    public function walk($node): void
    {
        if ($node === null) {
            return;
        }

        $this->walk($node->next);
    }
}
