
 <?php

 class uTesting extends FakeClass
  {
    const I_ = null;
    const hello = null;
    private $a;
    private $fake_variable = null;
    $no= null;
    $no_modifier = null;

    public function __construct($o)
    {
      try{

      }catch(\Exception $e){
      }

      $hello = false;
      if($hello == false){
         $this->no_modifier = 'helloworld';
      }else if ($hello === true){

        $this->no_ = 'hmm';

      }

      $this->fake_variable = 'hellworld';
      return '';
    }

    function test($a){

      if($a){

     }
      $this->does_not_exists();
      return 1;

    }

    public function no_return(bool $test):int{
      
      $this->faefa = "hae";
      if($test){
        return 200;
      }

      try{

      }catch(\RuntimeException $e){
        
      }
    }



 }
