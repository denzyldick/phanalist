<?php

final class SummaryRetainer
{
    public array $rows = [];

    public function use(array $items): void
    {
        foreach ($items as $item) {
            $this->rows = retain_known_helper($items);
        }
    }
}
