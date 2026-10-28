<?php

final class NodeWalker
{
    public function walk($node): void
    {
        switch ($node->localName) {
            case 'when':
                foreach ($node->childNodes as $node) {
                    $this->walk($node);
                }
                break;
        }
    }
}
