<?php

class ResourceMethodHelper
{
    public function nested(array $items): void
    {
        foreach ($items as $left) {
            foreach ($items as $right) {
                echo $left . $right;
            }
        }
    }
}
