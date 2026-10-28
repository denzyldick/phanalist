<?php

class BaseTask
{
    public function run(array $items): void
    {
    }
}

final class ChildTask extends BaseTask
{
    public function run(array $items): void
    {
        parent::run($items);
    }
}
